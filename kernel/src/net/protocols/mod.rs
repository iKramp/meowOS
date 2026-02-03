use crate::net::{packet::RawPacket, protocols::{arp::ArpHeader, ethernet::{EthernetHeader, parse_ethernet_frame}}};

pub(in crate::net) mod ethernet;
pub(in crate::net) mod arp;

pub(in crate::net) fn parse_net_packet(packet: &RawPacket, packet_type: NetLayer2Type) -> Option<Layer2Data> {
    let data = match packet_type {
        NetLayer2Type::Ethernet => Layer2Data::Ethernet(parse_ethernet_frame(packet)?),
    };
    Some(data)
}

fn parse_layer_3(packet: &RawPacket, offset: usize, layer_type: u32) -> (Layer3Data, u32) {
    match layer_type {
        0x0806 => arp::parse_arp(packet, offset), // ARP
        // 0x0800 => todo!("Implement IPv4"), // IPv4
        // 0x86DD => todo!("Implement IPv6"), // IPv6
        _ => (Layer3Data::Unknown, 0),
    }
}

#[derive(Debug)]
pub(in crate::net) enum Layer2Data {
    Ethernet(EthernetHeader),
}

#[derive(Debug)]
pub(in crate::net) enum Layer3Data {
    Unknown,
    Ipv4,
    Ipv6,
    Arp(ArpHeader),
}

pub enum NetLayer2Type {
    Ethernet,
}
