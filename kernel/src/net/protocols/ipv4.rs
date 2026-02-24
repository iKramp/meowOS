use std::cow::Acow;

use crate::net::{NetLayerType, ProtocolAddr, address_pair::AddressPair, packet::NetPacket, protocols::NetLayer, routing_tables};

pub(in crate::net) struct Ipv4Header {
    offset: u32,
    ihl: u8,
    diff_services: u8,
    total_length: u16,
    identification: u16,
    flags: Ipv4Flags,
    fragment_offset: u32,
    ttl: u8,
    protocol: u8,
    header_checksum: u16,
    source: Ipv4Address,
    destination: Ipv4Address,
    checksum_checked: bool,
}

bitfield::bitfield! {
    struct Ipv4Flags(u8);
    impl Debug;
    pub reserved, set_reserved: 2;
    pub dont_fragment, set_dont_fragment: 1;
    pub more_fragments, set_more_fragments: 0;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct Ipv4FlowId {
    address: AddressPair<Ipv4Address>,
    protocol: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::net) struct Ipv4Address(pub [u8; 4]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct Ipv4Network {
    address: Ipv4Address,
    mask: Ipv4Address,
}

impl Ipv4Network {
    pub fn contains(&self, ip: &Ipv4Address) -> bool {
        let ip_u32 = u32::from_be_bytes(ip.0);
        let network_u32 = u32::from_be_bytes(self.address.0);
        let mask_u32 = u32::from_be_bytes(self.mask.0);
        (ip_u32 & mask_u32) == (network_u32 & mask_u32)
    }

    pub fn prefix_len(&self) -> u32 {
        let mask_u32 = u32::from_be_bytes(self.mask.0);
        mask_u32.count_ones()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct Ipv4NetworkInterface {
    pub network: Ipv4Network,
    pub interface_ip: Ipv4Address,
}

pub(super) fn init() {
    todo!()
}

impl NetLayer for Ipv4Header {
    fn incoming_flow_direction(&self) -> crate::net::flow::IncomingFlowDirection {
        let is_own_ip = routing_tables::is_own_protocol_addr(&ProtocolAddr::Ipv4(self.destination));

        todo!()
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Ipv4
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn upper_layer_type(&self) -> NetLayerType {
        match self.protocol {
            1 => NetLayerType::Unknown, //ICMP, not yet supported
            2 => NetLayerType::Unknown, //IGMP, not yet supported
            6 => NetLayerType::Tcp,
            17 => NetLayerType::Udp,
            41 => NetLayerType::Ipv6,
            89 => NetLayerType::Unknown, //OSPF, not yet supported
            132 => NetLayerType::Unknown, //SCTP, not yet supported
            _ => NetLayerType::Unknown,
        }
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset + self.ihl as u32 * 4
    }

    fn bridge_to_out_set_layers(&self, out_layers: &mut std::vec::Vec<super::NetLayerFlowID>) {
        let flow_id = Ipv4FlowId {
            address: AddressPair::new(self.source, self.destination),
            protocol: self.protocol,
        };

        out_layers.push(super::NetLayerFlowID::Ipv4(flow_id));
    }
}

pub(super) fn parse_ipv4_packet(packet: &mut Acow<NetPacket>, offset: u32) -> Option<Ipv4Header> {
    let packet_len = packet.len();
    if packet_len < offset + 1 {
        return None;
    }
    packet.ensure_length(offset + 1);

    let v_ihl = packet.get_chunks()[0].data()[offset as usize];
    if v_ihl >> 4 != 4 {
        return None; // Not IPv4
    }

    let header_len = ((v_ihl & 0x0F) as u32) * 4;

    if packet_len < header_len {
        return None;
    }

    packet.ensure_length(header_len);
    let data = &packet.get_chunks()[0].data()[offset as usize..];
    let differentated_services = data[1] >> 2;
    let total_length = u16::from_be_bytes([data[2], data[3]]);
    let identification = u16::from_be_bytes([data[4], data[5]]);
    let flags = Ipv4Flags((data[6] & 0xE0) >> 5);
    let fragment_offset = ((u16::from_be_bytes([data[6], data[7]])) & 0x1FFF) as u32;
    let ttl = data[8];
    let protocol = data[9];
    let header_checksum = u16::from_be_bytes([data[10], data[11]]);
    let source = Ipv4Address([data[12], data[13], data[14], data[15]]);
    let destination = Ipv4Address([data[16], data[17], data[18], data[19]]);
    //ignore potential options

    Some(Ipv4Header {
        offset,
        ihl: v_ihl & 0x0F,
        diff_services: differentated_services,
        total_length,
        identification,
        flags,
        fragment_offset,
        ttl,
        protocol,
        header_checksum,
        source,
        destination,
        checksum_checked: false,
    })
}
