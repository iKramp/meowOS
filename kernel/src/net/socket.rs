use core::hash::{Hash, Hasher};
use std::{boxed::Box, queue::DataQueueHead, sync::rw_lock::RWSpinlock};

use crate::net::{self, address_pair::AddressPair, packet::ProcessedPacket, protocols::NetAddress};

const SOCKET_QUEUE_SIZE: usize = 1024;

pub struct NetSocket {
    addresses: Box<[AddressPair<NetAddress>]>,
    queue: RWSpinlock<DataQueueHead<ProcessedPacket>>,
}

impl NetSocket {
    pub fn new(addresses: Box<[AddressPair<NetAddress>]>) -> NetSocket {

        NetSocket {
            addresses,
            queue: RWSpinlock::new(DataQueueHead::new(SOCKET_QUEUE_SIZE)),
        }
    }

    pub fn get_addr_hash(&self) -> u64 {
        let mut hasher = unsafe { net::NET_HASHER.assume_init_ref().clone() };
        for addr in self.addresses.iter() {
            addr.hash(&mut hasher);
        }
        hasher.finish()
    }

    pub fn get_bind_addr_hash(&self) -> u64 {
        let mut hasher = unsafe { net::NET_HASHER.assume_init_ref().clone() };
        for addr in self.addresses.iter() {
            addr.source.hash(&mut hasher); //only local address is relevant for binding
        }
        hasher.finish()
    }
}
