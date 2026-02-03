mod net_queue;
mod packet;
mod protocols;

use std::println;

pub use packet::{NetPacket, RawNetDataChunk};
pub use protocols::NetLayer2Type;

pub fn debug_packet(packet: &NetPacket, packet_type: NetLayer2Type) {
    let parsed = protocols::parse_net_packet(packet.raw_data(), packet_type);
    println!("Parsed Packet: {:X?}", parsed);
    println!(level:info, "parsed_packet");
}
