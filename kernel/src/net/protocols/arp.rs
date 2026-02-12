use std::cow::Acow;

use crate::net::{
    NetLayerType,
    flow::IncomingFlowDirection,
    packet::RawPacket,
    protocols::{MacAddress, NetLayer}, routing_tables::{self, get_self_arp_entry},
};

#[derive(Debug, Clone)]
pub(in crate::net) struct ArpHeader {
    offset: u32,
    operation: u16,
    sender_hardware: HardwareAddr,
    sender_protocol: ProtocolAddr,
    target_hardware: HardwareAddr,
    target_protocol: ProtocolAddr,
}

impl NetLayer for ArpHeader {
    fn incoming_flow_direction(&self) -> IncomingFlowDirection {
        IncomingFlowDirection::Bridge
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Arp
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn upper_layer_type(&self) -> NetLayerType {
        NetLayerType::None
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset
    }

    fn action(&self) {
        routing_tables::update_arp_entry(self.sender_hardware.clone(), self.sender_protocol.clone());
        if self.operation == 2 { //response to some other request, ignore
            return; //drop packet
        }
        let Some(self_entry) = get_self_arp_entry(&self.target_protocol) else {
            return; //drop packet
        };
        self.operation = 2;
        core::mem::swap(&mut self.sender_protocol, &mut self.target_protocol);
        self.target_hardware = self.sender_hardware.clone();
        self.sender_hardware = self_entry;
        //continue routing
    }
}

#[derive(Debug, Clone)]
pub(in crate::net) enum HardwareAddr {
    Ethernet(MacAddress),
}

impl HardwareAddr {
    fn from_bytes(hardware_type: u16, data: &[u8]) -> Option<Self> {
        match hardware_type {
            1 => {
                let mut addr = MacAddress([0u8; 6]);
                addr.0.copy_from_slice(&data[0..6]);
                Some(HardwareAddr::Ethernet(addr))
            }
            _ => None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::net) enum ProtocolAddr {
    Ipv4([u8; 4]),
    Ipv6([u8; 16]), //not really used
}

impl ProtocolAddr {
    fn from_bytes(protocol_type: u16, data: &[u8]) -> Option<Self> {
        match protocol_type {
            0x0800 => {
                let mut addr = [0u8; 4];
                addr.copy_from_slice(&data[0..4]);
                Some(ProtocolAddr::Ipv4(addr))
            }
            0x86DD => {
                let mut addr = [0u8; 16];
                addr.copy_from_slice(&data[0..16]);
                Some(ProtocolAddr::Ipv6(addr))
            }
            _ => None
        }
    }
}

pub(in crate::net::protocols) fn parse_arp(packet: &mut Acow<RawPacket>, mut offset: usize) -> Option<ArpHeader> {
    packet.ensure_length(offset as u32 + 28);

    let arp_offset = offset as u32;
    let chunks = packet.get_chunks();
    let data = chunks[0].data();
    let hardware_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let protocol_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let hardware_size = data[offset] as usize;
    offset += 1;
    let protocol_size = data[offset] as usize;
    offset += 1;

    let operation = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let sender_hardware = HardwareAddr::from_bytes(hardware_type, &data[offset..(offset + hardware_size)])?;
    offset += hardware_size;

    let sender_protocol = ProtocolAddr::from_bytes(protocol_type, &data[offset..(offset + protocol_size)])?;
    offset += protocol_size;

    let target_hardware = HardwareAddr::from_bytes(hardware_type, &data[offset..(offset + hardware_size)])?;
    offset += hardware_size;

    let target_protocol = ProtocolAddr::from_bytes(protocol_type, &data[offset..(offset + hardware_size)])?;


    let arp_header = ArpHeader {
        offset: arp_offset,
        operation,
        sender_hardware,
        sender_protocol,
        target_hardware,
        target_protocol,
    };
    Some(arp_header)
}
