use core::{ptr::addr_of_mut, range::Range};
use std::println;

use crate::net::{
    packet::RawPacket,
    protocols::{Layer3Data, parse_layer_3},
};

#[derive(Debug)]
pub(in crate::net) struct EthernetHeader {
    offset: u32,
    trailer: Option<Range<u32>>,
    crc_offset: u32,
    source: [u8; 6],
    destination: [u8; 6],
    lower_type: u16,
    lower_data: Layer3Data,
}

pub(super) fn parse_ethernet_frame(packet: &RawPacket) -> Option<EthernetHeader> {
    let packet_len = packet.len();
    if packet_len < 14 {
        // Ethernet header + minimum payload + CRC
        println!("Ethernet frame too short: {}", packet_len);
        return None;
    }

    packet.ensure_length(14);
    let chunks = packet.get_chunks();
    let data = chunks[0].data();
    let mut destination: [u8; 6] = [0; 6];
    let mut source: [u8; 6] = [0; 6];
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().byte_add(0), addr_of_mut!(destination) as *mut u8, 6) };
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().byte_add(6), addr_of_mut!(source) as *mut u8, 6) };
    let lower_type = u16::from_be_bytes([data[12], data[13]]);

    let (lower_layer_data, mut lower_layer_len) = parse_layer_3(packet, 14, lower_type as u32);

    if matches!(lower_layer_data, Layer3Data::Unknown) {
        lower_layer_len = packet_len - 14 - 4; // Assume rest of packet minus CRC
    }

    let lower_layer_end = 14 + lower_layer_len;
    let trailer = if packet_len > lower_layer_end + 4 {
        Some(Range {
            start: lower_layer_end,
            end: packet_len - 4,
        })
    } else {
        None
    };

    Some(EthernetHeader {
        offset: 0,
        trailer,
        crc_offset: packet_len - 4,
        source,
        destination,
        lower_type,
        lower_data: lower_layer_data,
    })
}
