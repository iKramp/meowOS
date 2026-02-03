use core::mem::MaybeUninit;

use crate::net::{
    NetPacket,
    protocols::{self, NetLayerType},
};
use bitfield::bitfield;

enum HookStage {
    BadPacket,
    Inbound(NetLayerType),
    Outbound(NetLayerType),
}

enum RoutingStep {
    Parsed,
    SendOut,
}

bitfield! {
    pub struct FlowDirectionFlags(u32);
    impl Debug;
    pub layer_up, set_layer_up: 0;
    pub current_layer, set_current_layer: 1;
}

fn call_hooks(_packet: &mut NetPacket, stage: HookStage) {
    match stage {
        HookStage::BadPacket => {
            todo!()
        }
        HookStage::Inbound(_) => {
            todo!()
        }
        HookStage::Outbound(_) => {
            todo!()
        }
    }
}

fn process_next_step(
    packet: NetPacket,
    mut flow_direction: FlowDirectionFlags,
    current_layer_type: NetLayerType,
    upper_layer_type: NetLayerType,
    upper_layer_offset: usize,
) {
    let mut packet = MaybeUninit::new(packet);

    if flow_direction.current_layer() {
        let packet_here = unsafe { packet.assume_init() };
        packet = MaybeUninit::uninit();
        flow_direction.set_current_layer(false);

        let has_more_layers = flow_direction.0 != 0;
        if has_more_layers {
            packet = MaybeUninit::new(packet_here.clone());
        }

        process_bridge(packet_here, current_layer_type);

        if !has_more_layers {
            return;
        }
    }

    if flow_direction.layer_up() {
        let packet_here = unsafe { packet.assume_init() };
        flow_direction.set_layer_up(false);

        process_inbound_packet(packet_here, upper_layer_type, upper_layer_offset);
    }
}

fn process_inbound_packet(mut packet: NetPacket, layer_type: NetLayerType, layer_offset: usize) {
    if matches!(layer_type, NetLayerType::None) {
        return; //no more layers to process
    }

    let Some(parsed) = protocols::parse_layer(&packet.raw_data(), layer_type, layer_offset) else {
        call_hooks(&mut packet, HookStage::BadPacket);
        return; //bad packet, drop it
    };

    let is_known = parsed.is_known();

    packet.parsed_layers.push(parsed);

    call_hooks(&mut packet, HookStage::Inbound(layer_type));

    if !is_known {
        return; //unknown layer, stop processing
    }

    //Safety: parsed_layers just had a pushed, known layer
    let curr_layer = unsafe { packet.get_highest_layer().unwrap_unchecked() };

    let next_step = curr_layer.flow_direction();
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

/// Processing a packet to be sent out
/// This sets up the layer indicated by layer_type. If the layer already exists
/// (existing_layer_index is Some), it modifies that layer instead of creating a new one.
fn process_outbound_packet(mut _packet: NetPacket, _layer_type: NetLayerType, _existing_layer_index: Option<usize>) {
    todo!();

    //last action if packet is not dropped should be process_outbound_packet(packet, lower_layer_type);
}

/// Processing a packet on the same layer as before
/// This does not parse the packet in any way, but may do *something* with it
/// Examples: protocols like ARP or bridging packets
fn process_bridge(mut _packet: NetPacket, _layer_type: NetLayerType) {
    todo!();


    //last action if packet is not dropped should be process_outbound_packet(packet, layer_type);
}
