use crate::mdns::mDNSSender;
use async_channel::Receiver;
use bytes::Bytes;
use futures::SinkExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{error, info};

pub struct TunnelPeer {
    pub mdns_sender: Arc<Mutex<mDNSSender>>,
    pub channel_rx: Receiver<Vec<u8>>,
    pub tcp: Framed<TcpStream, LengthDelimitedCodec>,
    pub socket_addr: Option<SocketAddr>,
}

impl TunnelPeer {
    pub async fn select_run(self) {
        let TunnelPeer {
            mdns_sender,
            channel_rx,
            mut tcp,
            socket_addr,
        } = self;

        loop {
            tokio::select! {
                matched = channel_rx.recv() => {
                    match matched {
                        Ok(packet) => {
                            let bytes = Bytes::copy_from_slice(&packet);
                            if let Err(e) = tcp.send(bytes).await {
                                error!(?e, "tcp send err");
                                break;
                            }
                        },
                        Err(_) => break
                    }
                },
                matched = tcp.next() => {
                    match matched {
                        Some(Ok(packet)) => {
                            let mut lock = mdns_sender.lock().await;
                            if let Some(Err(e)) = lock.send(&packet.to_vec()) {
                                error!(?e, "mdns sender send err");
                                break;
                            }
                        },
                        Some(Err(e)) => {
                            error!(?e, "read buf error!");
                            break;
                        },
                        None => break
                    }
                },
            }
        }
        if let Some(addr) = socket_addr {
            info!(?addr, "peer closed!");
        } else {
            info!("peer closed!");
        }
    }
}
