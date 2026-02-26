use std::boxed::Box;

use crate::net::packet::NetPacketListNode;

pub struct NetQueueHead {
    first_packet: Option<Box<NetPacketListNode>>,
    last_packet: Option<*mut NetPacketListNode>,
    max_packets: usize,
    curr_packets: usize,
}

unsafe impl Send for NetQueueHead {} //only accessed through these functions

impl NetQueueHead {
    pub const fn new(max_packets: usize) -> Self {
        Self {
            first_packet: None,
            last_packet: None,
            max_packets,
            curr_packets: 0,
        }
    }

    pub fn push(&mut self, mut packet: Box<NetPacketListNode>) {
        while self.curr_packets >= self.max_packets {
            //drop oldest
            let _ = self.get_first();
        }

        let Some(last_packet_ptr) = self.last_packet else {
            let raw_ptr = packet.as_mut() as *mut NetPacketListNode;
            self.first_packet = Some(packet);
            self.last_packet = Some(raw_ptr);
            self.curr_packets = 1;
            return;
        };

        let last_packet = unsafe { &mut *last_packet_ptr };
        let new_raw_ptr = packet.as_mut() as *mut NetPacketListNode;
        last_packet.next_packet = Some(packet);
        self.last_packet = Some(new_raw_ptr);
        self.curr_packets += 1;
    }

    pub fn get_first(&mut self) -> Option<Box<NetPacketListNode>> {
        let mut dummy = Option::None;
        core::mem::swap(&mut dummy, &mut self.first_packet);
        let mut first_packet = dummy?;
        core::mem::swap(&mut first_packet.next_packet, &mut self.first_packet);

        if self.first_packet.is_none() {
            self.last_packet = None;
        }

        Some(first_packet)
    }

    pub fn append(&mut self, other: NetQueueHead) {
        if other.curr_packets == 0 {
            return;
        }

        let max_packets = self.max_packets;

        while self.curr_packets + other.curr_packets > self.max_packets && self.curr_packets > 0 {
            //drop oldest
            let _ = self.get_first();
        }

        if self.first_packet.is_none() {
            *self = other;
            self.max_packets = max_packets;
            while self.curr_packets > max_packets {
                let _ = self.get_first();
            }
            return;
        }

        let Some(last_packet_ptr) = self.last_packet else {
            unreachable!();
        };

        let last_packet = unsafe { &mut *last_packet_ptr };
        last_packet.next_packet = other.first_packet;
        self.last_packet = other.last_packet;
        self.curr_packets += other.curr_packets;
    }

    pub fn len(&self) -> usize {
        self.curr_packets
    }
}
