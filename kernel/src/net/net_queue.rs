use std::boxed::Box;

use crate::net::packet::NetPacket;

struct NetQueueHead {
    first_packet: Option<Box<NetPacket>>,
    last_packet: Option<*mut NetPacket>,
    max_packets: usize,
    curr_packets: usize,
}

impl NetQueueHead {
    fn new(max_packets: usize) -> Self {
        Self {
            first_packet: None,
            last_packet: None,
            max_packets,
            curr_packets: 0,
        }
    }

    fn push(&mut self, mut packet: Box<NetPacket>) {
        while self.curr_packets >= self.max_packets {
            //drop oldest
            let _ = self.get_first();
        }

        let Some(last_packet_ptr) = self.last_packet else {
            let raw_ptr = packet.as_mut() as *mut NetPacket;
            self.first_packet = Some(packet);
            self.last_packet = Some(raw_ptr);
            return;
        };

        let last_packet = unsafe { &mut *last_packet_ptr };
        let new_raw_ptr = packet.as_mut() as *mut NetPacket;
        last_packet.next_packet = Some(packet);
        self.last_packet = Some(new_raw_ptr);
    }

    fn get_first(&mut self) -> Option<Box<NetPacket>> {
        let mut dummy = Option::None;
        core::mem::swap(&mut dummy, &mut self.first_packet);
        let mut first_packet = dummy?;
        core::mem::swap(&mut first_packet.next_packet, &mut self.first_packet);

        if self.first_packet.is_none() {
            self.last_packet = None;
        }

        Some(first_packet)
    }
}
