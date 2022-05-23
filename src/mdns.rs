use dns_parser::Packet as mDNSPacket;
use pnet::datalink::{DataLinkReceiver, DataLinkSender};
use pnet::{
    datalink::{self, Channel::Ethernet, NetworkInterface},
    packet::{
        ethernet::{EtherTypes, EthernetPacket},
        ip::IpNextHeaderProtocols,
        ipv4::Ipv4Packet,
        udp::UdpPacket,
        Packet,
    },
};
use std::io;
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

/// An mDNS listener on a specific interface.
#[allow(non_camel_case_types)]
pub struct mDNSListener {
    pub eth_rx: Box<dyn DataLinkReceiver>,
    // `EthernetPacket` with `mDNS`
    pub channel_tx: UnboundedSender<Vec<u8>>,
    pub filter_domains: Vec<String>,
}

impl mDNSListener {
    /// Listen mDNS packet, than send `EthernetPacket` to channel
    pub fn listen(&mut self) {
        // mDNSPacket<'a>
        let mut mdns_buf = Vec::new();

        while let Ok(packet) = self.eth_rx.next() {
            if let Some(eth) = EthernetPacket::new(packet) {
                if let Some(mdns) = mdns_packet(&eth, &mut mdns_buf) {
                    if filter_packet(&mdns, &self.filter_domains) {
                        if self.channel_tx.send(packet.to_vec()).is_err() {
                            break;
                        }
                    }
                }
            };
        }
    }
}

/// An mDNS Sender on a specific interface.
#[allow(non_camel_case_types)]
pub struct mDNSSender {
    pub eth_tx: Box<dyn DataLinkSender>,
}

impl mDNSSender {
    /// packet is a `EthernetPacket` with `mDNS`
    pub fn send(&mut self, packet: &[u8]) -> Option<Result<(), io::Error>> {
        self.eth_tx.send_to(packet, None)
    }
}

pub fn pair(
    interface: &NetworkInterface,
    channel_tx: UnboundedSender<Vec<u8>>,
    filter_domains: Vec<String>,
) -> (mDNSSender, mDNSListener) {
    // Create a channel to receive on
    let (tx, rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("unhandled channel type"),
        Err(e) => panic!("unable to create channel: {}", e),
    };
    (
        mDNSSender { eth_tx: tx },
        mDNSListener {
            eth_rx: rx,
            channel_tx,
            filter_domains,
        },
    )
}

/// get multicast dns packet  
fn mdns_packet<'a>(ethernet: &EthernetPacket, buf: &'a mut Vec<u8>) -> Option<mDNSPacket<'a>> {
    fn ipv4_packet(payload: &[u8]) -> Option<Ipv4Packet> {
        let packet = Ipv4Packet::new(payload)?;
        if !packet.get_destination().is_multicast()
            || !matches!(packet.get_next_level_protocol(), IpNextHeaderProtocols::Udp)
        {
            return None;
        }
        Some(packet)
    }

    fn udp_packet(payload: &[u8]) -> Option<UdpPacket> {
        UdpPacket::new(payload)
    }

    match ethernet.get_ethertype() {
        EtherTypes::Ipv4 => {
            let ipv4_packet = ipv4_packet(ethernet.payload())?;
            let udp_packet = udp_packet(ipv4_packet.payload())?;
            *buf = udp_packet.payload().to_vec();
            mDNSPacket::parse(buf).ok()
        }
        _ => None,
    }
}

fn filter_packet(packet: &mDNSPacket, domains: &Vec<String>) -> bool {
    let question_matched = packet
        .questions
        .iter()
        .filter(|record| {
            let record_name = record.qname.to_string();
            let matched = domains.contains(&record_name);
            if matched {
                info!("found query packet, domain: {}", record_name);
            }
            matched
        })
        .count()
        > 0;

    let answer_matched = packet
        .answers
        .iter()
        .filter(|record| {
            let record_name = record.name.to_string();
            let matched = domains.contains(&record_name);

            if matched {
                info!(
                    "found answer packet, domain: {} at: {:?}",
                    record_name, &record.data
                );
            }
            matched
        })
        .count()
        > 0;

    question_matched || answer_matched
}
