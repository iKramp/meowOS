use std::{boxed::Box, cow::Acow, println, vec::Vec, w_lock_w_info};

use crate::net::{self, NetLayerType, NetPacketListNode, RoutingStep, address_pair::AddressPair, flow::{IncomingFlowDirection, LayerDownType, OutgoingFlowDirection}, hook::{HookResult, NET_HOOK_STORAGE}, packet::NetPacket, protocols::{NetLayer, NetLayerFlowID, ipv4::{Ipv4Address, Ipv4Flags, Ipv4FlowId, Ipv4FragmentInfo}}};


#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::net) struct IcmpFlowId {
    pub icmp_type: u8,
    pub code: u8,
    pub address: AddressPair<Ipv4Address>,
    pub data: u32,
    pub payload: Vec<u8>,
}


#[derive(Debug, Clone)]
pub(in crate::net) struct IcmpHeader {
    offset: u32,
    icmp_type: u8,
    code: u8,
    checksum: u16,
    data: u32,
    payload: Vec<u8>,
}

pub(super) fn init() {
    w_lock_w_info!(NET_HOOK_STORAGE).register_hook(icmp_in_hook, net::hook::HookStage::Inbound(NetLayerType::Icmp));
}

impl NetLayer for IcmpHeader {
    fn incoming_flow_direction(&self) -> IncomingFlowDirection {
        IncomingFlowDirection::Drop
    }

    fn current_layer_type(&self) -> NetLayerType {
        NetLayerType::Icmp
    }

    fn current_layer_offset(&self) -> u32 {
        self.offset
    }

    fn upper_layer_type(&self) -> NetLayerType {
        NetLayerType::None
    }

    fn upper_layer_offset(&self) -> u32 {
        self.offset + 8 + self.payload.len() as u32
    }

    fn bridge_to_out_set_layers(&self, _out_layers: &mut Vec<NetLayerFlowID>) {
        //shouldn't be here
    }
}

pub(in crate::net) fn parse_icmp(packet: &mut Acow<NetPacket>, offset: usize) -> Option<IcmpHeader> {
    packet.ensure_length(offset as u32 + 8);
    let len = packet.upper_layer_size?;

    let chunks = packet.get_chunks();
    let data = chunks[0].data();

    let icmp_type = data[offset];
    let code = data[offset + 1];
    let checksum = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
    let _data = u32::from_be_bytes([
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ]);

    let payload = data[offset + 8..offset + len].to_vec();

    let header = IcmpHeader {
        offset: offset as u32,
        icmp_type,
        code,
        checksum,
        data: _data,
        payload,
    };

    Some(header)
}

pub(in crate::net) fn construct_layer(packet: &mut Acow<NetPacket>) -> OutgoingFlowDirection {
    let Some(NetLayerFlowID::Icmp(flow_id)) = packet.layers_to_construct.pop() else {
        println!(level:error, "construct_layer called for Icmp but highest layer is not Icmp");
        return OutgoingFlowDirection::Drop;
    };

    let chunk_to_edit = packet.insert_chunk_front(8 + flow_id.payload.len() as u32);

    let chunk_data = chunk_to_edit.data_mut();

    chunk_data[0] = flow_id.icmp_type;
    chunk_data[1] = flow_id.code;
    chunk_data[2..4].copy_from_slice(&0u16.to_be_bytes());
    chunk_data[4..8].copy_from_slice(&flow_id.data.to_be_bytes());
    chunk_data[8..].copy_from_slice(&flow_id.payload);

    let checksum = net::compute_internet_checksum(&chunk_data[..8 + flow_id.payload.len()]);

    chunk_data[2..4].copy_from_slice(&checksum.to_be_bytes());

    packet.layers_to_construct.push(NetLayerFlowID::Ipv4(Ipv4FlowId {
        protocol: 1,
        address: flow_id.address,
        diff_services: 0,
        ttl: 64,
        fragment_info: Ipv4FragmentInfo {
            identification: 0,
            flags: Ipv4Flags(0),
            fragment_offset: 0,
        }
    }));

    OutgoingFlowDirection::LayerDown(LayerDownType::Normal(NetLayerType::Ipv4))
    
}

fn icmp_in_hook(packet: &mut NetPacketListNode) -> HookResult {
    println!("icmp in hook running");
    let icmp_layer = match packet
        .get_highest_layer()
        .and_then(|layer| (layer as &dyn std::any::Any).downcast_ref::<IcmpHeader>())
    {
        Some(layer) => layer,
        None => return HookResult::Drop,
    };

    let ipv4_layer = match packet
        .get_layer(1)
        .and_then(|layer| (layer as &dyn std::any::Any).downcast_ref::<net::protocols::ipv4::Ipv4Header>())
    {
        Some(layer) => layer,
        None => return HookResult::Drop,
    };

    match icmp_layer.icmp_type {
        8 => {
            println!(level:info, "ICMP Ping request");

            let mut layers = Vec::new();
            layers.push(NetLayerFlowID::Icmp(IcmpFlowId {
                icmp_type: 0,
                code: 0,
                address: ipv4_layer.address.reverse(),
                data: icmp_layer.data,
                payload: icmp_layer.payload.clone(),
            }));

            let mut new_packet = NetPacketListNode::new(Vec::new(), net::NetPacketSource::Other, RoutingStep::Outgoing, NetLayerType::Icmp);
            new_packet.data.layers_to_construct = layers;

            net::add_net_packet_to_queue(Box::new(new_packet));

            HookResult::Drop
        }
        0 => {
            println!(level:info, "ICMP Ping reply");

            HookResult::Drop
        }
        _ => {
            println!("Received ICMP packet with type {}, dropping", icmp_layer.icmp_type);
            HookResult::Drop
        }
    }
}
