mod flow;
mod net_queue;
mod packet;
mod protocols;
mod routing_tables;
mod hook;

pub use packet::{NetPacket, NetPacketSource, RawNetDataChunk};
pub use protocols::NetLayerType;

use crate::net::flow::process_inbound_packet;

pub type NicIdentifier = u32;

static NIC_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

pub fn requset_nic_identifier() -> NicIdentifier {
    NIC_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
}

pub fn debug_packet(packet: NetPacket) {
    let layer_type = packet.packet_type();
    // let parsed = protocols::parse_layer(packet.raw_data(), *packet.packet_type(), 0);
    // println!("Parsed Packet: {:X?}", parsed);
    // println!(level:info, "parsed_packet");
    process_inbound_packet(packet, layer_type, 0);
}
