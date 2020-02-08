use std::env;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use anyhow::Result;
use futures::future;
use tokio::prelude::*;
use tokio::{
    io::BufReader,
    net::{
        udp::{RecvHalf, SendHalf},
        UdpSocket,
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

    let app = Arc::new(ChatApp { peer: peer_addr });

    let send_task = tokio::spawn(app.clone().sender_task(send));

    let recv_task = tokio::spawn(app.clone().recv_task(recv));

    future::select(send_task, recv_task)
        .await
        .factor_first()
        .0??;

    Ok(())
}

struct ChatApp {
    peer: std::net::SocketAddr,
}

impl ChatApp {
    async fn sender_task(self: Arc<Self>, mut send: SendHalf) -> Result<()> {
        let mut stdin = BufReader::new(tokio::io::stdin()).lines();

        while let Some(line) = stdin.next_line().await? {
            send.send_to(line.as_bytes(), &self.peer).await?;
        }

        Ok(())
    }

    async fn recv_task(self: Arc<Self>, mut recv: RecvHalf) -> Result<()> {
        loop {
            let mut buffer = [0u8; 1024];
            let (recv_len, recv_from) = recv.recv_from(&mut buffer).await?;

            println!(
                "Got message from {}: {}",
                recv_from,
                String::from_utf8_lossy(&buffer[..recv_len])
            );
        }
    }
}
