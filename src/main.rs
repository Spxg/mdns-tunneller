pub mod config;
pub mod mdns;
pub mod tunnel;

use anyhow::Result;
use async_channel::Receiver;
use clap::Parser;
use pnet::datalink::{self, NetworkInterface};
use std::sync::Arc;
use std::thread;
use tokio::{net::TcpListener, sync::Mutex};
use tokio::{
    net::TcpStream,
    sync::mpsc::{self, UnboundedReceiver},
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{info, Level};
use tunnel::TunnelPeer;

use crate::config::get_filter_domains;

#[derive(Parser)]
enum Args {
    Server {
        #[clap(short, long)]
        addr: String,
        #[clap(short, long)]
        interface: String,
    },
    Client {
        #[clap(short, long)]
        addr: String,
        #[clap(short, long)]
        interface: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();
    let (is_client, addr, iface_name) = match args {
        Args::Server { addr, interface } => (false, addr, interface),
        Args::Client { addr, interface } => (true, addr, interface),
    };
    info!(?is_client, ?addr, ?iface_name);

    let interface_names_match = |iface: &NetworkInterface| iface.name == iface_name;

    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .filter(interface_names_match)
        .next()
        .unwrap_or_else(|| panic!("No such network interface: {}", iface_name));

    let (channel_tx, channel_rx) = mpsc::unbounded_channel();
    let (mdns_sender, mut mdns_listener) = mdns::pair(&interface, channel_tx, get_filter_domains());

    let mdns_sender = Arc::new(Mutex::new(mdns_sender));
    let channel_rx = forward(channel_rx);

    if is_client {
        let tcp = TcpStream::connect(&addr).await?;
        info!("connected");

        thread::spawn(move || mdns_listener.listen());
        let tunnel = TunnelPeer {
            mdns_sender,
            channel_rx,
            tcp: Framed::new(tcp, LengthDelimitedCodec::new()),
            socket_addr: None,
        };
        tunnel.select_run().await;
    } else {
        let listener = TcpListener::bind(&addr).await?;
        info!("start listening");

        thread::spawn(move || mdns_listener.listen());

        while let Ok((con, addr)) = listener.accept().await {
            info!(?addr, "connected");

            let mdns_sender = mdns_sender.clone();
            let channel_rx = channel_rx.clone();

            tokio::spawn(async move {
                let tunnel = TunnelPeer {
                    mdns_sender,
                    channel_rx,
                    tcp: Framed::new(con, LengthDelimitedCodec::new()),
                    socket_addr: Some(addr),
                };
                tunnel.select_run().await;
            });
        }
    }

    Ok(())
}

fn forward(mut sc_rx: UnboundedReceiver<Vec<u8>>) -> Receiver<Vec<u8>> {
    let (tx, rx) = async_channel::unbounded();
    tokio::spawn(async move {
        while let Some(packet) = sc_rx.recv().await {
            if tx.send(packet).await.is_err() {
                break;
            }
        }
    });
    rx
}
