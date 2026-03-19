use std::{cow::Acow, println, vec::Vec, w_lock_w_info};

use crate::net::{
    self, NetAddress, NetLayerType, RawNetDataChunk,
    address_pair::AddressPair,
    flow::{LayerDownType, OutgoingFlowDirection},
    hook::{self, HookResult},
    packet::NetPacket,
    protocols::{NetLayer, NetLayerFlowID},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::net) struct UdpPort(pub u16);

#[derive(Debug, Clone)]
pub(in crate::net) struct UdpHeader {
    pub offset: u32,
    pub ports: AddressPair<UdpPort>,
    pub length: u16,
    pub checksum: u16,
}

impl NetLayer for UdpHeader {
    fn incoming_flow_direction(&self) -> net::flow::IncomingFlowDirection {
        net::flow::IncomingFlowDirection::Bridge
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Udp
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn upper_layer_type(&self) -> NetLayerType {
        NetLayerType::None
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset + 8
    }

    fn bridge_to_out_set_layers(&self, _out_layers: &mut Vec<NetLayerFlowID>) {
        //shouldn't be here
    }
}

fn bridge_hook_function(packet: &mut Acow<NetPacket>) -> HookResult {
    //check for sockets that match

    println!("UDP packet received. Data: {:?}", packet);
    packet.print();

    HookResult::Drop //never bridge UDP
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct UdpFlowId {
    pub source_port: u16,
    pub dest_port: u16,
}

pub(super) fn init() {
    w_lock_w_info!(hook::NET_HOOK_STORAGE).register_hook(bridge_hook_function, hook::HookStage::Bridge(NetLayerType::Udp));
}

pub(super) fn parse_udp(packet: &mut Acow<NetPacket>, offset: usize) -> Option<UdpHeader> {
    packet.ensure_length(offset as u32 + 8);
    let chunks = packet.get_chunks();
    let data = chunks[0].data();

    let source_port = u16::from_be_bytes([data[offset], data[offset + 1]]);
    let dest_port = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
    let length = u16::from_be_bytes([data[offset + 4], data[offset + 5]]);
    let checksum = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);

    packet.append_address(AddressPair::new(
        NetAddress::UdpPort(UdpPort(source_port)),
        NetAddress::UdpPort(UdpPort(dest_port)),
    ));

    println!(
        "Parsed UDP header: source_port={}, dest_port={}, length={}, checksum={}",
        source_port, dest_port, length, checksum
    );

    Some(UdpHeader {
        offset: offset as u32,
        ports: AddressPair::new(UdpPort(source_port), UdpPort(dest_port)),
        length,
        checksum,
    })
}

pub(super) fn construct_layer(packet: &mut Acow<NetPacket>) -> OutgoingFlowDirection {
    let Some(NetLayerFlowID::Udp(data)) = packet.layers_to_construct.pop() else {
        println!(level:error, "construct_layer called for UDP but highest layer is not UDP");
        return OutgoingFlowDirection::Drop;
    };
    let Some(next_layer) = packet.layers_to_construct.last().cloned() else {
        println!(level:error, "construct_layer for UDP called but no next layer to determine checksum for");
        return OutgoingFlowDirection::Drop;
    };

    let len = 8 + packet.len() as u16;

    let mut new_chunk = RawNetDataChunk::allocate_new(8);
    let chunk_data = new_chunk.data_mut();
    chunk_data[0..2].copy_from_slice(&data.source_port.to_be_bytes());
    chunk_data[2..4].copy_from_slice(&data.dest_port.to_be_bytes());
    chunk_data[4..6].copy_from_slice(&len.to_be_bytes());
    chunk_data[6..8].copy_from_slice(&0u16.to_be_bytes()); //don't use checksum for now

    match next_layer {
        NetLayerFlowID::Ipv4(ipv4_flow_id) => {
            let mut checksum_arr = [0; 20];
            checksum_arr[0..4].copy_from_slice(&ipv4_flow_id.address.source.0);
            checksum_arr[4..8].copy_from_slice(&ipv4_flow_id.address.target.0);
            checksum_arr[8] = 0;
            checksum_arr[9] = 17; //udp protocol number
            checksum_arr[10..12].copy_from_slice(&len.to_be_bytes());
            checksum_arr[12..].copy_from_slice(&chunk_data[..8]);
            let mut checksum_vec = checksum_arr.to_vec();

            for chunk in packet.get_chunks() {
                checksum_vec.extend_from_slice(chunk.data());
            }

            let checksum = net::compute_internet_checksum(&checksum_vec);
            chunk_data[6..8].copy_from_slice(&checksum.to_be_bytes());
            packet.insert_existing_chunk(new_chunk);

            OutgoingFlowDirection::LayerDown(LayerDownType::Normal(NetLayerType::Ipv4))
        }
        NetLayerFlowID::Ipv6 => todo!(),
        _ => {
            println!(level:error, "construct_layer for UDP called but next layer is not IPv4 or IPv6");
            OutgoingFlowDirection::Drop
        }
    }
}
