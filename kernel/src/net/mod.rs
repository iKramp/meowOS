#![allow(deprecated)] //siphasher

mod address_pair;
mod flow;
mod hook;
mod packet;
mod protocols;
mod routing_tables;
mod socket;

use core::fmt::Debug;
use core::hash::SipHasher;
use core::mem::MaybeUninit;
use std::lock_w_info;
use std::println;
use std::sync::no_int_spinlock::NoIntSpinlock;

pub use flow::RoutingStep;
pub use packet::{PacketInRouting, RawNetDataChunk};
pub use protocols::MacAddress;
pub use protocols::NetLayerType;
pub use routing_tables::deregister_nic;
pub use routing_tables::register_nic;

use crate::net::protocols::arp::ProtocolAddr;
use crate::rand::rand_u64;
use crate::task_runner;
use std::queue::*;

pub type NicIdentifier = u32;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NicType {
    Ethernet,
    Ipv4,
}

const MAX_PACKETS: usize = 512;

static NIC_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

static mut NET_INITIALIZED: bool = false;

static NET_QUEUE: NoIntSpinlock<DataQueueHead<PacketInRouting>> = NoIntSpinlock::new(DataQueueHead::new(MAX_PACKETS));

static mut NET_HASHER: MaybeUninit<SipHasher> = MaybeUninit::uninit();

pub fn requset_nic_identifier() -> NicIdentifier {
    NIC_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
}

pub fn debug_packet(mut _packet: PacketInRouting, layer_type: NetLayerType) {
    println!("Packet at layer {:?}:", layer_type);
}

#[allow(clippy::upper_case_acronyms)]
pub trait NIC: Sync + Send {
    fn send_packet(&self, packet: PacketInRouting);
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
    unsafe {
        NET_HASHER = MaybeUninit::new(SipHasher::new_with_keys(rand_u64(), rand_u64()));
    }
    protocols::init();
    task_runner::add_repeating_task(process_packets);
    unsafe {
        NET_INITIALIZED = true;
    }
}

fn process_packets() {
    if !unsafe { NET_INITIALIZED } {
        return;
    }

    let mut new_queue = DataQueueHead::new(MAX_PACKETS);
    let mut process_queue = lock_w_info!(NET_QUEUE);
    println!("{}", process_queue.len());
    std::mem::swap(&mut new_queue, &mut process_queue);
    drop(process_queue);

    while let Some(packet) = new_queue.get_first() {
        println!("Processing packet");
        flow::process_packet_flow(packet);
    }
}

pub fn add_net_packet_to_queue(packet: PacketInRouting) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    lock_w_info!(NET_QUEUE).push(packet);
}

pub fn append_net_queue(other: DataQueueHead<PacketInRouting>) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    lock_w_info!(NET_QUEUE).append(other);
}

fn compute_internet_checksum(header: &[u8]) -> u16 {
    println!("Computing checksum for header: {:x?}", header);
    let mut sum = 0u32;
    for i in (0..header.len()).step_by(2) {
        let word = u16::from_be_bytes([header[i], header[i + 1]]) as u32;
        sum = sum.wrapping_add(word);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    let res = !(sum as u16);
    println!("Computed checksum: {:#x}", res);
    res
}
