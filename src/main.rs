use std::env;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use anyhow::{Error, Result};
use futures::try_join;
use serde::{Deserialize, Serialize};
use tokio::{
    io::BufReader,
    net::{
        udp::{RecvHalf, SendHalf},
        UdpSocket,
    },
    prelude::*,
    sync::{
        mpsc::{channel, Receiver, Sender},
        RwLock,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    let host = env::var("HOST").unwrap();
    let peer_host = env::var("PEER").unwrap();

    let addr = (host.as_str(), 1337)
        .to_socket_addrs()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    let peer_addr = (peer_host.as_str(), 1337)
        .to_socket_addrs()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    let socket = UdpSocket::bind(addr).await?;

    let (recv, send) = socket.split();

    let (mut sender_tx, sender_rx) = channel::<Msg>(10);
    sender_tx.send(Msg::Hello).await?;

    let app = Arc::new(ChatApp {
        addr,
        peers: RwLock::new(vec![peer_addr]),
        sender_tx: sender_tx.clone(),
    });

    let send_task = spawn(app.clone().sender_task(sender_rx, send));
    let stdin_read_task = spawn(app.clone().stdin_read_task());
    let recv_task = spawn(app.clone().recv_task(recv));

    try_join!(send_task, stdin_read_task, recv_task)?;

    Ok(())
}

async fn spawn<F>(fut: F) -> Result<()>
where
    F: std::future::Future<Output = Result<()>> + Send + 'static,
{
    tokio::spawn(fut).await??;
    Ok(())
}

struct ChatApp {
    addr: SocketAddr,
    peers: RwLock<Vec<SocketAddr>>,
    sender_tx: Sender<Msg>,
}

#[derive(Debug, Deserialize, Serialize)]
enum Msg {
    ChatMsg(String),
    Hello,
    NewPeer(SocketAddr),
}

impl ChatApp {
    async fn sender_task(
        self: Arc<Self>,
        mut sender_rx: Receiver<Msg>,
        mut send: SendHalf,
    ) -> Result<()> {
        while let Some(msg) = sender_rx.recv().await {
            for peer in &*self.peers.read().await {
                send.send_to(&serde_cbor::to_vec(&msg)?, peer).await?;
            }
        }
        Ok::<(), Error>(())
    }

    async fn stdin_read_task(self: Arc<Self>) -> Result<()> {
        let mut stdin = BufReader::new(tokio::io::stdin()).lines();

        while let Some(line) = stdin.next_line().await? {
            self.sender_tx.clone().send(Msg::ChatMsg(line)).await?;
        }

        Ok(())
    }

    async fn recv_task(self: Arc<Self>, mut recv: RecvHalf) -> Result<()> {
        let mut buffer = [0u8; 1024];
        loop {
            let (recv_len, recv_from) = recv.recv_from(&mut buffer).await?;
            if !self.peers.read().await.contains(&recv_from) {
                self.peers.write().await.push(recv_from);
                self.sender_tx.clone().send(Msg::NewPeer(recv_from)).await?;
            }
            let msg = match serde_cbor::from_slice::<Msg>(&buffer[..recv_len]) {
                Err(err) => {
                    eprintln!("Error decoding message: {:?}", err);
                    continue;
                }
                Ok(msg) => msg,
            };

            match msg {
                Msg::ChatMsg(val) => {
                    println!("{}: {}", recv_from, val);
                }
                Msg::Hello => {
                    eprintln!("{} connected", recv_from);
                    // Make sure they knows about all peers
                    // OBS: This is O(n^2), should probably only send to the new peer, yeah yeah
                    for peer in &*self.peers.read().await {
                        self.sender_tx
                            .clone()
                            .send(Msg::NewPeer(peer.clone()))
                            .await?;
                    }
                }
                Msg::NewPeer(new_peer) => {
                    if new_peer != self.addr && !self.peers.read().await.contains(&new_peer) {
                        eprintln!("{} joined by msg from {}", new_peer, recv_from);
                        self.peers.write().await.push(new_peer);
                    }
                }
            }
        }
    }
}
