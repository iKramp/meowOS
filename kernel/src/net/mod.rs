mod flow;
mod net_queue;
mod packet;
mod protocols;
mod routing_tables;
mod hook;
mod address_pair;

use core::fmt::Debug;
use std::boxed::Box;
use std::lock_w_info;
use std::println;
use std::sync::no_int_spinlock::NoIntSpinlock;

pub use packet::{NetPacketListNode, RawNetDataChunk, NetPacketSource};
pub use protocols::NetLayerType;
pub use routing_tables::register_nic;
pub use routing_tables::deregister_nic;
pub use flow::{process_packet_flow, RoutingStep};
pub use protocols::MacAddress;

use crate::net::net_queue::NetQueueHead;
use crate::net::protocols::arp::ProtocolAddr;

pub type NicIdentifier = u32;
pub enum NicType {
    Ethernet,
}

static NIC_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

static mut NET_INITIALIZED: bool = false;

static NET_QUEUE: NoIntSpinlock<NetQueueHead> = NoIntSpinlock::new(NetQueueHead::new(512));

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
    fn nic_type(&self) -> NicType;
}

impl Debug for dyn NIC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NIC {{ id: {} }}", self.get_identifier())
    }
}

pub fn init() {
    println!("Initializing net subsystem");
    protocols::init();
    unsafe { NET_INITIALIZED = true; }
}

pub fn add_net_packet_to_queue(packet: Box<NetPacketListNode>) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    lock_w_info!(NET_QUEUE).push(packet);
}

pub fn append_net_queue(other: NetQueueHead) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    lock_w_info!(NET_QUEUE).append(other);
}
