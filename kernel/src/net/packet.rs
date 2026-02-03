use std::{
    boxed::Box,
    mem_utils::{PhysAddr, translate_phys_virt_addr},
    r_lock_w_info,
    sync::{
        arc::Arc,
        rw_lock::{RWLockModeRead, RWLockModeWrite, RWSpinlock, RWSpinlockGuard},
    },
    vec::Vec,
    w_lock_w_info,
};

use crate::{
    memory::physical_allocator,
    net::{NicIdentifier, protocols::{NetLayer, NetLayerData, NetLayerType}},
    proc::Pid,
};

#[derive(Debug, Clone)]
pub enum NetPacketSource {
    Nic(NicIdentifier),
    Proc(Pid),
    Other,
}

#[derive(Clone)]
pub struct NetPacket {
    raw_data: Arc<RawPacket>,
    pub(in crate::net) parsed_layers: Vec<NetLayerData>,
    pub(in crate::net) next_packet: Option<Box<NetPacket>>,
}

impl NetPacket {
    pub fn new(raw_data: Vec<RawNetDataChunk>, packet_type: NetLayerType, source: NetPacketSource) -> Self {
        let raw_data = Arc::new(RawPacket::new(raw_data, packet_type, source));

        NetPacket {
            raw_data,
            parsed_layers: Vec::new(),
            next_packet: None,
        }
    }

    pub fn from_single(raw_data: RawNetDataChunk, packet_type: NetLayerType, source: NetPacketSource) -> Self {
        let mut tmp_vec = Vec::new();
        tmp_vec.push(raw_data);
        let raw_data = Arc::new(RawPacket::new(tmp_vec, packet_type, source));

        NetPacket {
            raw_data,
            parsed_layers: Vec::new(),
            next_packet: None,
        }
    }

    pub(in crate::net) fn raw_data(&self) -> &RawPacket {
        &self.raw_data
    }

    pub(in crate::net) fn packet_type(&self) -> &NetLayerType {
        &self.raw_data.packet_type
    }

    pub(in crate::net) fn get_highest_layer(&self) -> Option<&dyn NetLayer> {
        Some(self.parsed_layers.last()?.get().expect("can't call get_highest_layer on unparsed layer"))
    }

    //next step
}

#[derive(Debug)]
pub(in crate::net) struct RawPacket {
    chunks: RWSpinlock<Vec<RawNetDataChunk>>,
    source: NetPacketSource,
    length: u32,
    packet_type: NetLayerType,
}

impl RawPacket {
    pub fn new(data: Vec<RawNetDataChunk>, packet_type: NetLayerType, source: NetPacketSource) -> Self {
        let len = data.iter().fold(0, |a, b| a + b.length);

        RawPacket {
            chunks: RWSpinlock::new(data),
            packet_type,
            source,
            length: len,
        }
    }

    pub fn linearize(&self) {
        let mut vec = w_lock_w_info!(self.chunks);
        if vec.len() <= 1 {
            return;
        }
        let len = vec.iter().fold(0, |a, b| a + b.length);
        let pages = len.div_ceil(4096);
        let phys_addr = physical_allocator::allocate_contiguius_high(pages as u64);
        let virt_addr = translate_phys_virt_addr(phys_addr);
        let mut curr_offset = 0;
        for chunk in vec.iter() {
            let chunk_virt = translate_phys_virt_addr(chunk.data);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    chunk_virt.0 as *const u8,
                    (virt_addr.0 as *mut u8).byte_add(curr_offset),
                    chunk.length as usize,
                )
            };
            curr_offset += chunk.length as usize;
        }
        *vec = Vec::new();
        vec.push(RawNetDataChunk {
            data: phys_addr,
            length: len,
        });
    }

    pub fn ensure_length(&self, len: u32) {
        let vec = r_lock_w_info!(self.chunks);
        if vec[0].len() < len {
            self.linearize();
        }
    }

    pub fn len(&self) -> u32 {
        self.length
    }

    pub fn get_chunks(&self) -> RWSpinlockGuard<Vec<RawNetDataChunk>, RWLockModeRead> {
        r_lock_w_info!(self.chunks)
    }

    pub fn get_chunks_mut(&mut self) -> RWSpinlockGuard<Vec<RawNetDataChunk>, RWLockModeWrite> {
        w_lock_w_info!(self.chunks)
    }

    pub fn packet_type(&self) -> &NetLayerType {
        &self.packet_type
    }
}

//must be contigious
//owns the data
#[derive(Debug)]
pub struct RawNetDataChunk {
    data: PhysAddr,
    length: u32,
}

impl RawNetDataChunk {
    pub fn new(data: PhysAddr, length: u32) -> Self {
        RawNetDataChunk { data, length }
    }

    pub fn len(&self) -> u32 {
        self.length
    }

    pub fn data(&self) -> &[u8] {
        let ptr = translate_phys_virt_addr(self.data).0 as *const u8;
        unsafe { core::slice::from_raw_parts(ptr, self.length as usize) }
    }
}

impl Drop for RawNetDataChunk {
    fn drop(&mut self) {
        if self.data.0 == 0 {
            return;
        }

        let pages = self.length.div_ceil(4096);
        for i in 0..pages {
            unsafe {
                physical_allocator::deallocate_frame(PhysAddr(self.data.0 + (i as u64) * 4096));
            }
        }
    }
}
