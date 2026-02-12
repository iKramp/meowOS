use std::println;

use crate::net::{
    NetPacketListNode, hook::{HookStage, call_hooks}, protocols::{self, NetLayerType}
};

enum RoutingStep {
    Parsed,
    SendOut,
}

pub(in crate::net) enum IncomingFlowDirection {
    LayerUp,
    Bridge,
    Both,
}

fn process_next_step(
    packet: NetPacketListNode,
    flow_direction: IncomingFlowDirection,
    current_layer_type: NetLayerType,
    upper_layer_type: NetLayerType,
    upper_layer_offset: usize,
) {
    match flow_direction {
        IncomingFlowDirection::LayerUp => {
            process_inbound_packet(packet, upper_layer_type, upper_layer_offset);
        }
        IncomingFlowDirection::Bridge => {
            //process the packet on the same layer, then stop processing
            process_bridge(packet, current_layer_type);
        }
        IncomingFlowDirection::Both => {
            let packet_clone = packet.clone();
            process_bridge(packet, current_layer_type);
            process_inbound_packet(packet_clone, upper_layer_type, upper_layer_offset);
        }
    }
}

pub(in crate::net) fn process_inbound_packet(mut packet: NetPacketListNode, layer_type: NetLayerType, layer_offset: usize) {
    if matches!(layer_type, NetLayerType::None) {
        return; //no more layers to process
    }

    let Some(parsed) = protocols::parse_layer(&mut packet.raw_data, layer_type, layer_offset) else {
        call_hooks(&mut packet, HookStage::BadPacket);
        return; //bad packet, drop it
    };

    let is_known = parsed.is_known();

    packet.raw_data.parsed_layers.push(parsed);

    call_hooks(&mut packet, HookStage::Inbound(layer_type));

    if !is_known {
        return; //unknown layer, stop processing
    }

    //Safety: parsed_layers just had a pushed, known layer
    let curr_layer = unsafe { packet.get_highest_layer().unwrap_unchecked() };

    let next_step = curr_layer.incoming_flow_direction();
    let upper_layer_type = curr_layer.upper_layer_type();
    let upper_layer_offset = curr_layer.upper_layer_offset() as usize;

    process_next_step(
        packet,
        next_step,
        layer_type,
        upper_layer_type,
        upper_layer_offset,
    );
}

/// Processing a packet on the same layer as before
/// This does not parse the packet in any way, but may do *something* with it
/// Examples: protocols like ARP or bridging packets
fn process_bridge(mut packet: NetPacketListNode, _layer_type: NetLayerType) {

    let layer = unsafe { packet.get_highest_layer().unwrap_unchecked() };
    layer.action();


    //last action if packet is not dropped should be process_outbound_packet(packet, layer_type);
}

/// Processing a packet to be sent out
/// This sets up the layer indicated by layer_type. If the layer already exists
/// (existing_layer_index is Some), it modifies that layer instead of creating a new one.
fn process_outbound_packet(mut _packet: NetPacketListNode, _layer_type: NetLayerType, _existing_layer_index: Option<usize>) {
    todo!();

    //last action if packet is not dropped should be process_outbound_packet(packet, lower_layer_type);
}
