#![allow(deprecated)] //siphasher

mod address_pair;
mod flow;
mod hook;
mod packet;
mod protocols;
mod routing_tables;
mod socket;

use core::fmt::Debug;
use core::hash::Hash;
use core::hash::Hasher;
use core::hash::SipHasher;
use core::mem::MaybeUninit;
use std::boxed::Box;
use std::cow::Acow;
use std::lock_w_info;
use std::println;
use std::sync::arc::Arc;
use std::sync::no_int_spinlock::NoIntSpinlock;
use std::vec::Vec;

pub use flow::RoutingStep;
pub use packet::RawNetDataChunk;
pub use protocols::MacAddress;
pub use protocols::NetLayerType;
pub use routing_tables::deregister_nic;
pub use routing_tables::register_nic;

use crate::net::address_pair::AddressPair;
use crate::net::packet::NetPacket;
use crate::net::packet::NetPacketSource;
use crate::net::packet::PacketInRouting;
use crate::net::protocols::NetAddress;
use crate::net::protocols::arp::ProtocolAddr;
use crate::net::protocols::ipv4::Ipv4Address;
use crate::net::socket::NetSocket;
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

pub(in crate::net) fn hash_addr_slice(addrs: &[AddressPair<NetAddress>]) -> u64 {
    let mut hasher = unsafe { NET_HASHER.assume_init_ref().clone() };
    for addr in addrs {
        addr.hash(&mut hasher);
    }
    hasher.finish()
}

pub(in crate::net) fn hash_bind_addr_slice(addrs: &[AddressPair<NetAddress>]) -> u64 {
    let mut hasher = unsafe { NET_HASHER.assume_init_ref().clone() };
    for addr in addrs {
        addr.source.hash(&mut hasher); //only local address is relevant for binding
    }
    hasher.finish()
}

pub fn requset_nic_identifier() -> NicIdentifier {
    NIC_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
}

#[allow(clippy::upper_case_acronyms)]
pub trait NIC: Sync + Send {
    fn send_packet(&self, packet: Vec<RawNetDataChunk>);
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
    std::mem::swap(&mut new_queue, &mut process_queue);
    drop(process_queue);

    while let Some(packet) = new_queue.get_first() {
        flow::process_packet_flow(packet);
    }
}

pub(in crate::net) fn add_net_packet_to_queue(packet: PacketInRouting) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    lock_w_info!(NET_QUEUE).push(packet);
}

pub fn append_raw_net_queue(
    mut other: DataQueueHead<Vec<RawNetDataChunk>>,
    nic_id: NicIdentifier,
    routing_step: RoutingStep,
    layer: NetLayerType,
) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    let mut net_queue = lock_w_info!(NET_QUEUE);
    while let Some(packet) = other.get_first() {
        let packet_in_routing = PacketInRouting {
            data: Acow::new(NetPacket::new(packet, NetPacketSource::Nic(nic_id))),
            routing_step,
            layer,
        };
        net_queue.push(packet_in_routing);
    }
}

pub(in crate::net) fn append_net_queue(other: DataQueueHead<PacketInRouting>) {
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
