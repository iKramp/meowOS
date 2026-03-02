use core::any::Any;
use std::{cow::Acow, println, vec::Vec};

use crate::net::{
    flow::{IncomingFlowDirection, OutgoingFlowDirection},
    packet::NetPacket,
    protocols::{
        arp::{ArpFlowId, ArpHeader}, ethernet::{EthernetFlowId, EthernetHeader, parse_ethernet_frame}, icmp::{IcmpFlowId, IcmpHeader}, ipv4::{Ipv4FlowId, Ipv4Header, Ipv4Network}, udp::{UdpFlowId, UdpHeader}
    },
};

pub(in crate::net) mod arp;
pub(in crate::net) mod ethernet;
pub(in crate::net) mod ipv4;
pub(in crate::net) mod icmp;
pub(in crate::net) mod udp;

pub(in crate::net) fn init() {
    arp::init();
    ethernet::init();
    ipv4::init();
    icmp::init();
    udp::init();
}

pub(in crate::net) fn parse_layer(
    packet: &mut Acow<NetPacket>,
    packet_type: NetLayerType,
    offset: usize,
) -> Option<NetLayerData> {
    let data = match packet_type {
        NetLayerType::Ethernet => NetLayerData::Ethernet(parse_ethernet_frame(packet)?),
        NetLayerType::Arp => NetLayerData::Arp(arp::parse_arp(packet, offset)?),
        NetLayerType::Ipv4 => NetLayerData::Ipv4(ipv4::parse_ipv4_packet(packet, offset)?),
        NetLayerType::Icmp => NetLayerData::Icmp(icmp::parse_icmp(packet, offset)?),
        NetLayerType::Udp => NetLayerData::Udp(udp::parse_udp(packet, offset)?),
        NetLayerType::Unknown => NetLayerData::Unknown(UnknownLayer { offset }),
        NetLayerType::None => NetLayerData::None,
        _ => return None,
    };
    println!("Parsed layer: {:?} at offset {}", packet_type, offset);
    Some(data)
}

pub(in crate::net) fn construct_layer(packet: &mut Acow<NetPacket>, layer_type: NetLayerType) -> OutgoingFlowDirection {
    match layer_type {
        NetLayerType::Ethernet => ethernet::construct_layer(packet),
        NetLayerType::Arp => arp::construct_layer(packet),
        NetLayerType::Ipv4 => ipv4::construct_layer(packet),
        NetLayerType::Icmp => icmp::construct_layer(packet),
        NetLayerType::Udp => udp::construct_layer(packet),
        _ => panic!("construct_layer not implemented for layer type {:?}", layer_type),
    }
}

pub(in crate::net) trait NetLayer: Any {
    fn incoming_flow_direction(&self) -> IncomingFlowDirection;
    fn current_layer_type(&self) -> NetLayerType;
    fn current_layer_offset(&self) -> u32;
    fn upper_layer_type(&self) -> NetLayerType;
    fn upper_layer_offset(&self) -> u32;
    fn bridge_to_out_set_layers(&self, out_layers: &mut Vec<NetLayerFlowID>);
}

#[derive(Debug, Clone)]
pub(in crate::net) enum NetLayerData {
    Unparsed,
    Unknown(UnknownLayer),
    None, //for example above TCP, no longer a kernel thing
    Ethernet(EthernetHeader),
    Ipv4(Ipv4Header),
    Icmp(IcmpHeader),
    Ipv6,
    Arp(ArpHeader),
    Tcp,
    Udp(UdpHeader),
}

impl NetLayerData {
    pub fn is_known(&self) -> bool {
        match self {
            NetLayerData::Unparsed | NetLayerData::Unknown(_) => false,
            NetLayerData::None => panic!("NetLayerData::None should short circuit before is_known() is called"),
            _ => true,
        }
    }

    pub fn get(&self) -> Option<&dyn NetLayer> {
        match self {
            NetLayerData::Ethernet(header) => Some(header),
            NetLayerData::Arp(header) => Some(header),
            NetLayerData::Ipv4(header) => Some(header),
            NetLayerData::Icmp(header) => Some(header),
            NetLayerData::Udp(header) => Some(header),
            _ => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut dyn NetLayer> {
        match self {
            NetLayerData::Ethernet(header) => Some(header),
            NetLayerData::Arp(header) => Some(header),
            NetLayerData::Ipv4(header) => Some(header),
            NetLayerData::Icmp(header) => Some(header),
            NetLayerData::Udp(header) => Some(header),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetLayerType {
    Unparsed,
    Unknown,
    None,
    Ethernet,
    Ipv4,
    Icmp,
    Ipv6,
    Arp,
    Tcp,
    Udp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) enum NetLayerFlowID {
    Ethernet(EthernetFlowId),
    Arp(ArpFlowId),
    Ipv4(Ipv4FlowId),
    Icmp(IcmpFlowId),
    Ipv6,
    Tcp,
    Udp(UdpFlowId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::net) enum NetAddress {
    Ipv4(Ipv4Network),
    Mac(MacAddress),
}

impl NetAddress {
    pub fn into_hardware(self) -> Option<arp::HardwareAddr> {
        match self {
            NetAddress::Mac(mac) => Some(arp::HardwareAddr::Ethernet(mac)),
            _ => None,
        }
    }

    pub fn into_protocol(self) -> Option<arp::ProtocolAddr> {
        match self {
            NetAddress::Ipv4(ipv4) => Some(arp::ProtocolAddr::Ipv4(ipv4.address)),
            _ => None,
        }
    }

    pub fn from_hardware(hardware: &arp::HardwareAddr) -> Option<Self> {
        match hardware {
            arp::HardwareAddr::Ethernet(mac) => Some(NetAddress::Mac(*mac)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub fn is_broadcast(&self) -> bool {
        self.0.iter().all(|&b| b == 0xFF)
    }
}

#[derive(Debug, Clone)]
pub(in crate::net) struct UnknownLayer {
    offset: usize,
}
