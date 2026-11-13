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
use std::println;
use std::ring_queue::RingQueue;
use std::sync::once_lock::OnceLock;
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
use crate::rand::rand_u64;
use crate::task_runner;

pub type NicIdentifier = u32;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NicType {
    Ethernet,
    Ipv4,
}

const MAX_PACKETS: usize = 512;

static NIC_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

static mut NET_INITIALIZED: bool = false;

static NET_QUEUE: OnceLock<RingQueue<PacketInRouting>> = OnceLock::new();

static mut NET_HASHER: MaybeUninit<SipHasher> = MaybeUninit::uninit();

fn get_net_queue() -> &'static RingQueue<PacketInRouting> {
    NET_QUEUE.get_or_init(|| RingQueue::new(MAX_PACKETS as u16))
}

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
    task_runner::add_repeating_task(Box::new(process_packets));

    unsafe {
        NET_INITIALIZED = true;
    }
}

fn process_packets() {
    if !unsafe { NET_INITIALIZED } {
        return;
    }

    let net_queue = get_net_queue();
    while let Some(packet) = net_queue.get_first() {
        flow::process_packet_flow(packet);
    }
}

pub(in crate::net) fn add_net_packet_to_queue(packet: PacketInRouting) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    get_net_queue().push(packet);
}

pub fn append_raw_net_queue(
    other: impl IntoIterator<Item = Vec<RawNetDataChunk>>,
    nic_id: NicIdentifier,
    routing_step: RoutingStep,
    layer: NetLayerType,
) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    let net_queue = get_net_queue();
    let mapped = other.into_iter().map(|packet| PacketInRouting {
        data: Acow::new(NetPacket::new(packet, NetPacketSource::Nic(nic_id))),
        routing_step,
        layer,
    });
    net_queue.append_iter(mapped);
}

pub(in crate::net) fn append_net_queue(other: RingQueue<PacketInRouting>) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    get_net_queue().append(other);
}

pub(in crate::net) fn append_net_queue_iter(iter: impl IntoIterator<Item = PacketInRouting>) {
    if !unsafe { NET_INITIALIZED } {
        return;
    }
    get_net_queue().append_iter(iter);
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
