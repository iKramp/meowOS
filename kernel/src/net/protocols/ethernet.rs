use core::ptr::addr_of_mut;
use std::{cow::Acow, println};

use crate::net::{
    packet::NetPacket,
    protocols::{MacAddress, NetLayer, NetLayerType}, routing_tables::is_own_mac,
};

#[derive(Debug, Clone)]
pub(in crate::net) struct EthernetHeader {
    offset: u32,
    crc_offset: u32,
    source: MacAddress,
    destination: MacAddress,
    lower_type: u16,
}

impl NetLayer for EthernetHeader {
    fn incoming_flow_direction(&self) -> crate::net::flow::IncomingFlowDirection {
        let destination_is_broadcast = self.destination.is_broadcast();
        let destination_is_me = is_own_mac(&self.destination);

        match (destination_is_broadcast, destination_is_me) {
            (true, _) => crate::net::flow::IncomingFlowDirection::Both,
            (false, true) => crate::net::flow::IncomingFlowDirection::LayerUp,
            (false, false) => crate::net::flow::IncomingFlowDirection::Bridge,
        }
    }

    fn upper_layer_type(&self) -> NetLayerType {
        match self.lower_type {
            0x0800 => super::NetLayerType::Ipv4,
            0x86DD => super::NetLayerType::Unknown, // IPv6 is not supported yet
            0x0806 => super::NetLayerType::Arp,
            _ => super::NetLayerType::Unknown,
        }
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset + 14
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Ethernet
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }
}

pub(super) fn parse_ethernet_frame(packet: &mut Acow<NetPacket>) -> Option<EthernetHeader> {
    let packet_len = packet.len();
    if packet_len < 14 {
        // Ethernet header + minimum payload + CRC
        println!("Ethernet frame too short: {}", packet_len);
        return None;
    }

    packet.ensure_length(14);
    let chunks = packet.get_chunks();
    let data = chunks[0].data();
    let mut destination = MacAddress([0; 6]);
    let mut source = MacAddress([0; 6]);
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().byte_add(0), addr_of_mut!(destination) as *mut u8, 6) };
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().byte_add(6), addr_of_mut!(source) as *mut u8, 6) };
    let lower_type = u16::from_be_bytes([data[12], data[13]]);

    Some(EthernetHeader {
        offset: 0,
        crc_offset: packet_len - 4,
        source,
        destination,
        lower_type,
    })
}
