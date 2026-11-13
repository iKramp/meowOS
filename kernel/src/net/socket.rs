use core::sync::atomic::AtomicU64;
use std::{boxed::Box, ring_queue::RingQueue};

use crate::net::{self, address_pair::AddressPair, packet::ProcessedPacket, protocols::NetAddress};

const SOCKET_QUEUE_SIZE: u16 = 1024;
static SOCKET_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct NetSocket {
    addresses: Box<[AddressPair<NetAddress>]>,
    queue: RingQueue<ProcessedPacket>,
    id: u64,
}

impl NetSocket {
    pub fn new(addresses: Box<[AddressPair<NetAddress>]>) -> NetSocket {
        NetSocket {
            addresses,
            queue: RingQueue::new(SOCKET_QUEUE_SIZE),
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
        self.queue.push(packet);
    }

    pub fn get_packet(&self) -> Option<ProcessedPacket> {
        self.queue.get_first()
    }
}
