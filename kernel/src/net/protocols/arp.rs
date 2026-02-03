use std::{boxed::Box, vec::Vec};

use crate::net::{packet::RawPacket, protocols::{MacAddress, NetLayerData}};

#[derive(Debug, Clone)]
pub(in crate::net) struct ArpHeader {
    operation: u16,
    sender_hardware: HardwareType,
    sender_protocol: ProtocolType,
    target_hardware: HardwareType,
    target_protocol: ProtocolType,
}

#[derive(Debug, Clone)]
enum HardwareType {
    Ethernet(MacAddress),
    Unknown((u16, Box<[u8]>)),
}

impl HardwareType {
    fn from_bytes(hardware_type: u16, data: Box<[u8]>) -> Self {
        match hardware_type {
            1 => {
                let mut addr = [0u8; 6];
                addr.copy_from_slice(&data[0..6]);
                HardwareType::Ethernet(addr)
            }
            _ => HardwareType::Unknown((hardware_type, data)),
        }
    }
}

#[derive(Debug, Clone)]
enum ProtocolType {
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
    Unknown((u16, Box<[u8]>)),
}

impl ProtocolType {
    fn from_bytes(protocol_type: u16, data: Box<[u8]>) -> Self {
        match protocol_type {
            0x0800 => {
                let mut addr = [0u8; 4];
                addr.copy_from_slice(&data[0..4]);
                ProtocolType::Ipv4(addr)
            }
            0x86DD => {
                let mut addr = [0u8; 16];
                addr.copy_from_slice(&data[0..16]);
                ProtocolType::Ipv6(addr)
            }
            _ => ProtocolType::Unknown((protocol_type, data)),
        }
    }
}

pub(in crate::net::protocols) fn parse_arp(packet: &RawPacket, mut offset: usize) -> Option<ArpHeader> {
    packet.ensure_length(offset as u32 + 28);

    let chunks = packet.get_chunks();
    let data = chunks[0].data();
    let hardware_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let protocol_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let hardware_size = data[offset];
    offset += 1;
    let protocol_size = data[offset];
    offset += 1;

    let operation = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let mut sender_harware_addr = Vec::with_capacity(hardware_size as usize);
    sender_harware_addr.extend_from_slice(&data[offset..(offset + hardware_size as usize)]);
    offset += hardware_size as usize;

    let mut sender_protocol_addr = Vec::with_capacity(protocol_size as usize);
    sender_protocol_addr.extend_from_slice(&data[offset..(offset + protocol_size as usize)]);
    offset += protocol_size as usize;

    let mut target_hardware_addr = Vec::with_capacity(hardware_size as usize);
    target_hardware_addr.extend_from_slice(&data[offset..(offset + hardware_size as usize)]);
    offset += hardware_size as usize;

    let mut target_protocol_addr = Vec::with_capacity(protocol_size as usize);
    target_protocol_addr.extend_from_slice(&data[offset..(offset + protocol_size as usize)]);

    let sender_hardware = HardwareType::from_bytes(hardware_type, sender_harware_addr.into_boxed_slice());
    let sender_protocol = ProtocolType::from_bytes(protocol_type, sender_protocol_addr.into_boxed_slice());
    let target_hardware = HardwareType::from_bytes(hardware_type, target_hardware_addr.into_boxed_slice());
    let target_protocol = ProtocolType::from_bytes(protocol_type, target_protocol_addr.into_boxed_slice());

    let arp_header = ArpHeader {
        operation,
        sender_hardware,
        sender_protocol,
        target_hardware,
        target_protocol,
    };
    Some(arp_header)
}
