# UdpChat

Simple unreliable chat using UDP, no server, peers are gossiping about new clients.
Experimental, just for testing out tokio.

```
HOST=127.0.0.1 PEER=127.0.0.2 cargo run
HOST=127.0.0.2 PEER=127.0.0.1 cargo run
HOST=127.0.0.3 PEER=127.0.0.1 cargo run
```
