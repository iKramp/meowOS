use core::sync::atomic::AtomicU64;
use std::{boxed::Box, lock_w_info, queue::DataQueueHead, sync::no_int_spinlock::NoIntSpinlock};

use crate::net::{self, address_pair::AddressPair, packet::ProcessedPacket, protocols::NetAddress};

const SOCKET_QUEUE_SIZE: usize = 1024;
static SOCKET_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct NetSocket {
    addresses: Box<[AddressPair<NetAddress>]>,
    queue: NoIntSpinlock<DataQueueHead<ProcessedPacket>>,
    id: u64,
}

impl NetSocket {
    pub fn new(addresses: Box<[AddressPair<NetAddress>]>) -> NetSocket {
        NetSocket {
            addresses,
            queue: NoIntSpinlock::new(DataQueueHead::new(SOCKET_QUEUE_SIZE)),
            id: SOCKET_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst),
        }
    }

    pub fn get_addr_hash(&self) -> u64 {
        net::hash_addr_slice(&self.addresses)
    }

    pub fn get_bind_addr_hash(&self) -> u64 {
        net::hash_bind_addr_slice(&self.addresses)
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn addresses(&self) -> &[AddressPair<NetAddress>] {
        &self.addresses
    }

    pub fn push_packet(&self, packet: ProcessedPacket) {
        lock_w_info!(self.queue).push(packet)
    }

    pub fn get_packet(&self) -> Option<ProcessedPacket> {
        lock_w_info!(self.queue).get_first()
    }
}
