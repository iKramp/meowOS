mod flow;
mod net_queue;
mod packet;
mod protocols;
mod routing_tables;
mod hook;
mod address_pair;

use core::fmt::Debug;
use std::println;

pub use packet::{NetPacketListNode, RawNetDataChunk, NetPacketSource};
pub use protocols::NetLayerType;
pub use routing_tables::register_nic;
pub use routing_tables::deregister_nic;
pub use flow::{process_packet_flow, RoutingStep};
pub use protocols::MacAddress;

use crate::net::protocols::arp::ProtocolAddr;

pub type NicIdentifier = u32;

static NIC_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

static IPV4_ADDR: ProtocolAddr = ProtocolAddr::Ipv4([192, 168, 0, 1]);

pub fn requset_nic_identifier() -> NicIdentifier {
    NIC_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
}

pub fn debug_packet(mut _packet: NetPacketListNode, layer_type: NetLayerType) {
    println!("Packet at layer {:?}:", layer_type);
}

#[allow(clippy::upper_case_acronyms)]
pub trait NIC: Sync + Send {
    fn send_packet(&self, packet: NetPacketListNode);
    fn get_identifier(&self) -> NicIdentifier;
}

impl Debug for dyn NIC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NIC {{ id: {} }}", self.get_identifier())
    }
}

pub fn init() {
    println!("Initializing net subsystem");
    protocols::arp::init();
}
