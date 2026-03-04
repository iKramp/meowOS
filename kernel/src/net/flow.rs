use std::{cow::Acow, println, sync::arc::Arc, vec::Vec};

use crate::net::{
    NIC, PacketInRouting, hook::{HookFilter, HookStage, call_hooks}, packet::{NetPacket, NetPacketSource}, protocols::{self, NetLayerType}
};

#[derive(Debug, Clone, Copy)]
pub enum RoutingStep {
    Incoming,
    Bridge,
    Outgoing,
}

#[derive(Debug)]
pub(in crate::net) enum IncomingFlowDirection {
    LayerUp(NetLayerType, usize), //upper layer type and offset
    Bridge,
    Both(NetLayerType, usize), //upper layer type and offset
    Drop,
}

#[derive(Debug)]
pub(in crate::net) enum LayerDownType {
    Normal(NetLayerType),
    Nic(Arc<dyn NIC>),
}

#[derive(Debug)]
pub(in crate::net) enum OutgoingFlowDirection {
    LayerDown(LayerDownType),
    Drop,
}

pub fn process_packet_flow(packet: PacketInRouting) {
    if !unsafe { crate::net::NET_INITIALIZED } {
        return;
    }
    let mut process_queue = Vec::new();

    println!("len of packet data: {}", packet.data.len());

    process_queue.push((0, packet));

    while let Some((current_layer_offset, mut packet)) = process_queue.pop() {
        let routing_step = packet.routing_step;
        let layer_type = packet.layer;
        match routing_step {
            RoutingStep::Incoming => {
                let proc_res = process_inbound_packet(&mut packet.data, layer_type, current_layer_offset);
                match proc_res {
                    IncomingFlowDirection::LayerUp(next_layer_type, next_layer_offset) => {
                        packet.routing_step = RoutingStep::Incoming;
                        packet.layer = next_layer_type;
                        process_queue.push((next_layer_offset, packet));
                    }
                    IncomingFlowDirection::Bridge => {
                        packet.routing_step = RoutingStep::Bridge;
                        packet.layer = layer_type;
                        process_queue.push((0, packet));
                    }
                    IncomingFlowDirection::Both(next_layer_type, next_layer_offset) => {
                        let mut cloned_packet = packet.clone();
                        cloned_packet.routing_step = RoutingStep::Incoming;
                        cloned_packet.layer = next_layer_type;
                        process_queue.push((next_layer_offset, cloned_packet));

                        packet.routing_step = RoutingStep::Bridge;
                        packet.layer = layer_type;
                        process_queue.push((0, packet));
                    }
                    IncomingFlowDirection::Drop => {}
                }
            }
            RoutingStep::Bridge => {
                let hook_filter = process_bridge(&mut packet.data, layer_type);
                if matches!(hook_filter, HookFilter::Continue) {
                    packet.routing_step = RoutingStep::Outgoing;
                    packet.layer = layer_type;
                    process_queue.push((0, packet));
                }
            }
            RoutingStep::Outgoing => {
                let out_res = process_outbound_packet(&mut packet.data, layer_type);
                match out_res {
                    OutgoingFlowDirection::LayerDown(LayerDownType::Normal(net_layer_type)) => {
                        packet.routing_step = RoutingStep::Outgoing;
                        packet.layer = net_layer_type;

                        process_queue.push((0, packet))
                    }
                    OutgoingFlowDirection::LayerDown(LayerDownType::Nic(nic)) => nic.send_packet(packet.into_raw_data()),
                    OutgoingFlowDirection::Drop => {}
                }
            }
        }
    }
}

fn process_inbound_packet(
    packet: &mut Acow<NetPacket>,
    layer_type: NetLayerType,
    layer_offset: usize,
) -> IncomingFlowDirection {
    println!(
        "processing inbound packet at layer {:?} with offset {}",
        layer_type, layer_offset
    );
    if matches!(layer_type, NetLayerType::None) {
        println!("packet has no more layers to process, dropping");
        return IncomingFlowDirection::Drop; //no more layers to process
    }

    let Some(parsed) = protocols::parse_layer(packet, layer_type, layer_offset) else {
        println!("failed to parse layer, dropping packet");
        call_hooks(packet, HookStage::BadPacket);
        return IncomingFlowDirection::Drop; //bad packet, drop it
    };

    let is_known = parsed.is_known();

    packet.parsed_layers.push(parsed);

    if matches!(call_hooks(packet, HookStage::Inbound(layer_type)), HookFilter::Drop) {
        println!("hook decided to drop the packet, dropping");
        return IncomingFlowDirection::Drop; //hook decided to drop the packet
    }

    if !is_known {
        println!("unknown layer type, dropping packet");
        return IncomingFlowDirection::Drop; //unknown layer, stop processing
    }

    //Safety: parsed_layers just had a pushed, known layer
    let curr_layer = unsafe { packet.get_highest_layer().unwrap_unchecked() };

    let res = curr_layer.incoming_flow_direction();
    println!("successfully processed inbound packet, next step: {:?}", res);
    res
}

/// Processing a packet on the same layer as before
/// This does not parse the packet in any way, but may do *something* with it
/// Examples: protocols like ARP/TCP/UDP or bridging packets
fn process_bridge(packet: &mut Acow<NetPacket>, layer_type: NetLayerType) -> HookFilter {
    println!("processing bridge packet at layer {:?}", layer_type);
    match call_hooks(packet, HookStage::Bridge(layer_type)) {
        HookFilter::Continue => {
            let original_packet = packet.clone();

            packet.bridge_to_out_set_layers();
            let top_layer = packet.get_highest_layer().expect("bridge with no layer");
            let top_layer_offset = top_layer.upper_layer_offset();
            let _ = packet.nuke_lower_layers(top_layer_offset);
            packet.reset_packet();
            packet.source = NetPacketSource::OtherPacket(original_packet);
            println!("successfully processed bridge, moving to outbound processing");
            HookFilter::Continue
        }
        HookFilter::Drop => {
            println!("hook decided to drop the packet, dropping");
            HookFilter::Drop
        }
    }
}

/// Processing a packet to be sent out
fn process_outbound_packet(packet: &mut Acow<NetPacket>, layer_type: NetLayerType) -> OutgoingFlowDirection {
    println!("processing outbound packet at layer {:?}", layer_type);
    if matches!(call_hooks(packet, HookStage::Outbound(layer_type)), HookFilter::Drop) {
        println!("hook decided to drop the packet, dropping");
        return OutgoingFlowDirection::Drop; //hook decided to drop the packet
    }
    let res = protocols::construct_layer(packet, layer_type);
    println!("successfully processed outbound packet, next step: {:?}", res);
    res
}
