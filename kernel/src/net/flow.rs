use std::{println, sync::arc::Arc, vec::Vec};

use crate::net::{
    NIC, NetPacketListNode,
    hook::{HookFilter, HookStage, call_hooks},
    protocols::{self, NetLayerType},
};

pub enum RoutingStep {
    Incoming,
    Bridge,
    Outgoing,
    Loopback,
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
    Loopback,
    Both(NetLayerType),
    Drop,
}

pub fn process_packet_flow(packet: NetPacketListNode, initial_routing_step: RoutingStep, initial_layer: NetLayerType) {
    if !unsafe { crate::net::NET_INITIALIZED } {
        return;
    }
    let mut process_queue = Vec::new();

    println!("len of packet data: {}", packet.data.len());

    process_queue.push(((initial_routing_step, initial_layer, 0), packet));

    while let Some(((routing_step, current_layer_type, current_layer_offset), mut packet)) = process_queue.pop() {
        match routing_step {
            RoutingStep::Incoming => {
                let proc_res = process_inbound_packet(&mut packet, current_layer_type, current_layer_offset);
                match proc_res {
                    IncomingFlowDirection::LayerUp(next_layer_type, next_layer_offset) => {
                        process_queue.push(((RoutingStep::Incoming, next_layer_type, next_layer_offset), packet));
                    }
                    IncomingFlowDirection::Bridge => {
                        process_queue.push(((RoutingStep::Bridge, current_layer_type, 0), packet));
                    }
                    IncomingFlowDirection::Both(next_layer_type, next_layer_offset) => {
                        process_queue.push(((RoutingStep::Incoming, next_layer_type, next_layer_offset), packet.clone()));
                        process_queue.push(((RoutingStep::Bridge, current_layer_type, 0), packet));
                    }
                    IncomingFlowDirection::Drop => {}
                }
            }
            RoutingStep::Bridge => {
                let hook_filter = process_bridge(&mut packet, current_layer_type);
                if matches!(hook_filter, HookFilter::Continue) {
                    process_queue.push(((RoutingStep::Outgoing, current_layer_type, 1), packet));
                }
            }
            RoutingStep::Outgoing => {
                let bridged = current_layer_offset == 1; //hacky way to track if this packet was bridged or not
                let out_res = process_outbound_packet(&mut packet, current_layer_type, bridged);
                match out_res {
                    OutgoingFlowDirection::LayerDown(LayerDownType::Normal(net_layer_type)) => {
                        process_queue.push(((RoutingStep::Outgoing, net_layer_type, 0), packet))
                    }
                    OutgoingFlowDirection::LayerDown(LayerDownType::Nic(nic)) => nic.send_packet(packet),
                    OutgoingFlowDirection::Loopback => {
                        process_queue.push(((RoutingStep::Loopback, current_layer_type, 0), packet))
                    }
                    OutgoingFlowDirection::Both(net_layer_type) => {
                        process_queue.push(((RoutingStep::Outgoing, net_layer_type, 0), packet.clone()));
                        process_queue.push(((RoutingStep::Loopback, current_layer_type, 0), packet));
                    }
                    OutgoingFlowDirection::Drop => {}
                }
            }
            RoutingStep::Loopback => {
                let hook_filter = process_loopback(&mut packet, current_layer_type);
                if matches!(hook_filter, HookFilter::Continue) {
                    process_queue.push(((RoutingStep::Outgoing, current_layer_type, 0), packet));
                }
            }
        }
    }
}

fn process_inbound_packet(
    packet: &mut NetPacketListNode,
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

    let Some(parsed) = protocols::parse_layer(&mut packet.data, layer_type, layer_offset) else {
        println!("failed to parse layer, dropping packet");
        call_hooks(packet, HookStage::BadPacket);
        return IncomingFlowDirection::Drop; //bad packet, drop it
    };

    let is_known = parsed.is_known();

    packet.data.parsed_layers.push(parsed);

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
fn process_bridge(packet: &mut NetPacketListNode, layer_type: NetLayerType) -> HookFilter {
    println!("processing bridge packet at layer {:?}", layer_type);
    match call_hooks(packet, HookStage::Bridge(layer_type)) {
        HookFilter::Continue => {
            packet.data.bridge_to_out_set_layers();
            let top_layer = packet.get_highest_layer().expect("bridge with no layer");
            let top_layer_offset = top_layer.current_layer_offset();
            let _ = packet.data.nuke_lower_layers(top_layer_offset);
            packet.data.reset_packet();
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
fn process_outbound_packet(packet: &mut NetPacketListNode, layer_type: NetLayerType, bridged: bool) -> OutgoingFlowDirection {
    println!("processing outbound packet at layer {:?}, bridged: {}", layer_type, bridged);
    if matches!(call_hooks(packet, HookStage::Outbound(layer_type)), HookFilter::Drop) {
        println!("hook decided to drop the packet, dropping");
        return OutgoingFlowDirection::Drop; //hook decided to drop the packet
    }
    let res = protocols::construct_layer(&mut packet.data, layer_type, bridged);
    println!("successfully processed outbound packet, next step: {:?}", res);
    res
}

fn process_loopback(packet: &mut NetPacketListNode, layer_type: NetLayerType) -> HookFilter {
    //This only allows the most basic processing. No layers are parsed at this point. Lower layers
    //are also nuked from exiting bridge
    println!("processing loopback packet at layer {:?} ", layer_type);
    call_hooks(packet, HookStage::Loopback(layer_type))
}
