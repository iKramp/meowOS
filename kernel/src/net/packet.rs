use std::{
    cow::Acow,
    mem_utils::{PhysAddr, translate_phys_virt_addr},
    println,
    vec::Vec,
};

use crate::{
    memory::physical_allocator,
    net::{
        self, NetLayerType, NicIdentifier, RoutingStep,
        address_pair::AddressPair,
        protocols::{NetAddress, NetLayer, NetLayerData, NetLayerFlowID},
    },
    proc::Pid,
};

#[derive(Debug, Clone)]
pub(in crate::net) enum NetPacketSource {
    Nic(NicIdentifier),
    Proc(Pid),
    OtherPacket(Acow<NetPacket>),
    Other,
}

#[derive(Debug)]
pub(in crate::net) struct PacketInRouting {
    pub(in crate::net) data: Acow<NetPacket>,
    pub(in crate::net) routing_step: RoutingStep,
    pub(in crate::net) layer: NetLayerType,
}

#[derive(Debug, Clone)]
pub struct ProcessedPacket {
    data: Acow<NetPacket>,
    user_data_start: u32,
}

impl PacketInRouting {
    pub(in crate::net) fn new(
        raw_data: Vec<RawNetDataChunk>,
        source: NetPacketSource,
        initial_routing_step: RoutingStep,
        initial_layer: NetLayerType,
    ) -> Self {
        let raw_data = Acow::new(NetPacket::new(raw_data, source));

        PacketInRouting {
            data: raw_data,
            routing_step: initial_routing_step,
            layer: initial_layer,
        }
    }

    pub fn from_nic_single_chunk(
        raw_data: RawNetDataChunk,
        nic_identifier: NicIdentifier,
        initial_routing_step: RoutingStep,
        initial_layer: NetLayerType,
    ) -> Self {
        let mut tmp_vec = Vec::new();
        tmp_vec.push(raw_data);
        let raw_data = Acow::new(NetPacket::new(tmp_vec, NetPacketSource::Nic(nic_identifier)));

        PacketInRouting {
            data: raw_data,
            routing_step: initial_routing_step,
            layer: initial_layer,
        }
    }

    pub(in crate::net) fn from_net_packet(
        packet: Acow<NetPacket>,
        initial_routing_step: RoutingStep,
        initial_layer: NetLayerType,
    ) -> Self {
        PacketInRouting {
            data: packet,
            routing_step: initial_routing_step,
            layer: initial_layer,
        }
    }

    pub fn into_raw_data(mut self) -> Vec<RawNetDataChunk> {
        let mut empty_vec = Vec::new();
        std::mem::swap(&mut self.data.chunks, &mut empty_vec);
        empty_vec
    }
}

impl Clone for PacketInRouting {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            routing_step: self.routing_step,
            layer: self.layer,
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::net) struct NetPacket {
    chunks: Vec<RawNetDataChunk>,
    pub parsed_layers: Vec<NetLayerData>,
    length: u32,
    pub source: NetPacketSource,
    /// Last element refers to the current layer (where the packet is being processed)
    /// After each layer is constructed, the last element is popped. If there is nothing left, the
    /// layer must either make up data, or reject the packet
    pub layers_to_construct: Vec<NetLayerFlowID>,
    pub upper_layer_size: Option<usize>,
    addresses: Vec<AddressPair<NetAddress>>,
}

impl NetPacket {
    pub fn new(data: Vec<RawNetDataChunk>, source: NetPacketSource) -> Self {
        let len = data.iter().fold(0, |a, b| a + b.length);

        NetPacket {
            chunks: data,
            parsed_layers: Vec::new(),
            length: len,
            layers_to_construct: Vec::new(),
            source,
            upper_layer_size: None,
            addresses: Vec::new(),
        }
    }

    pub fn into_processed(self: Acow<Self>) -> ProcessedPacket {
        let user_data_offset = self.get_highest_layer().map(|l| l.upper_layer_offset()).unwrap_or(0);

        ProcessedPacket {
            data: self,
            user_data_start: user_data_offset,
        }
    }

    pub fn get_highest_layer(&self) -> Option<&dyn NetLayer> {
        Some(
            self.parsed_layers
                .last()?
                .get()
                .expect("can't call get_highest_layer on unparsed layer"),
        )
    }

    pub fn get_highest_layer_mut(&mut self) -> Option<&mut dyn NetLayer> {
        Some(
            self.parsed_layers
                .last_mut()?
                .get_mut()
                .expect("can't call get_highest_layer on unparsed layer"),
        )
    }

    pub fn get_layer(&self, index_from_top: usize) -> Option<&dyn NetLayer> {
        Some(
            self.parsed_layers
                .get(self.parsed_layers.len() - 1 - index_from_top)?
                .get()
                .expect("can't call get_layer on unparsed layer"),
        )
    }

    pub fn get_addresses(&self) -> &Vec<AddressPair<NetAddress>> {
        &self.addresses
    }

    pub fn append_address(&mut self, addr: AddressPair<NetAddress>) {
        self.addresses.push(addr);
    }

    pub fn get_incoming_address_hash(&self) -> u64 {
        net::hash_addr_slice(&self.addresses.iter().map(AddressPair::reverse).collect::<Vec<_>>())
    }

    pub fn get_incoming_bind_address_hash(&self) -> u64 {
        net::hash_bind_addr_slice(&self.addresses.iter().map(AddressPair::reverse).collect::<Vec<_>>())
    }

    pub fn linearize(&mut self) {
        let vec = &mut self.chunks;
        if vec.len() <= 1 {
            return;
        }
        let len = vec.iter().fold(0, |a, b| a + b.length);
        let pages = len.div_ceil(4096);
        let phys_addr = physical_allocator::allocate_contiguous(pages);
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

    pub fn ensure_length(self: &mut Acow<Self>, len: u32) {
        if self.chunks[0].len() < len {
            self.linearize();
        }
    }

    pub fn len(&self) -> u32 {
        self.length
    }

    pub fn get_chunks(&self) -> &Vec<RawNetDataChunk> {
        &self.chunks
    }

    pub fn get_chunks_mut(&mut self) -> &mut Vec<RawNetDataChunk> {
        &mut self.chunks
    }

    pub fn reset_packet(&mut self) {
        self.parsed_layers.clear();
    }

    pub fn bridge_to_out_set_layers(&mut self) {
        let Some(highest_layer) = self.parsed_layers.last() else {
            return;
        };
        let Some(highest_layer) = highest_layer.get() else {
            return;
        };
        highest_layer.bridge_to_out_set_layers(&mut self.layers_to_construct);
    }

    pub fn nuke_lower_layers(&mut self, offset_to_keep: u32) -> Option<()> {
        let mut curr_off = 0;
        loop {
            let first_chunk = self.chunks.first()?;
            if curr_off + first_chunk.len() <= offset_to_keep {
                curr_off += first_chunk.len();
                self.chunks.remove(0);
            } else {
                break;
            }
        }
        self.length -= curr_off;
        let to_shift = offset_to_keep - curr_off;
        if to_shift > 0 {
            let first_chunk = self.chunks.first_mut()?;
            let chunk_virt = translate_phys_virt_addr(first_chunk.data);
            unsafe {
                core::ptr::copy(
                    chunk_virt.0 as *const u8,
                    (chunk_virt.0 as *mut u8).byte_add(to_shift as usize),
                    (first_chunk.len() - to_shift) as usize,
                )
            };
            let new_len = first_chunk.len() - to_shift;
            first_chunk.truncate(new_len);
            self.length -= to_shift;
        }
        Some(())
    }

    pub fn truncate(&mut self, new_len: u32) {
        println!("truncating packet from length {} to {}", self.length, new_len);
        if new_len >= self.length {
            return;
        }
        let to_delete = self.length - new_len;
        self.length = new_len;
        let mut deleted = 0;
        while deleted < to_delete {
            let Some(last_chunk) = self.chunks.last_mut() else {
                break;
            };
            if last_chunk.len() <= to_delete - deleted {
                deleted += last_chunk.len();
                self.chunks.pop();
            } else {
                last_chunk.truncate(last_chunk.len() - (to_delete - deleted));
                break;
            }
        }
    }

    pub fn insert_chunk_front(&mut self, length: u32) -> &mut RawNetDataChunk {
        let pages = length.div_ceil(4096);
        let phys_addr = physical_allocator::allocate_contiguous(pages);
        let chunk = RawNetDataChunk::new(phys_addr, length);
        self.chunks.insert(0, chunk);
        self.length += length;
        unsafe { self.chunks.first_mut().unwrap_unchecked() }
    }

    pub fn insert_existing_chunk(&mut self, chunk: RawNetDataChunk) {
        self.length += chunk.len();
        self.chunks.insert(0, chunk);
    }

    pub fn print(&self) {
        println!(
            "NetPacket: length {}, source {:?}, layers_to_construct: {:?}, parsed_layers: {:?}",
            self.length, self.source, self.layers_to_construct, self.parsed_layers
        );
        println!("Chunks:");
        for chunk in &self.chunks {
            println!("{:?}", chunk.data());
        }
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

    pub fn allocate_new(length: u32) -> Self {
        let pages = length.div_ceil(4096);
        let phys_addr = physical_allocator::allocate_contiguous(pages);
        RawNetDataChunk { data: phys_addr, length }
    }

    pub fn len(&self) -> u32 {
        self.length
    }

    pub fn phys_addr(&self) -> PhysAddr {
        self.data
    }

    pub fn data(&self) -> &[u8] {
        let ptr = translate_phys_virt_addr(self.data).0 as *const u8;
        unsafe { core::slice::from_raw_parts(ptr, self.length as usize) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        let ptr = translate_phys_virt_addr(self.data).0 as *mut u8;
        unsafe { core::slice::from_raw_parts_mut(ptr, self.length as usize) }
    }

    pub fn truncate(&mut self, new_len: u32) {
        if new_len >= self.length {
            return;
        }
        let pages_before = self.length.div_ceil(4096);
        self.length = new_len;
        let pages_after = self.length.div_ceil(4096);
        for i in pages_after..pages_before {
            unsafe {
                physical_allocator::deallocate_frame(PhysAddr(self.data.0 + (i as u64) * 4096));
            }
        }
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

impl Clone for RawNetDataChunk {
    fn clone(&self) -> Self {
        if self.data.0 == 0 {
            return Self {
                data: self.data,
                length: self.length,
            };
        }
        let pages = self.length.div_ceil(4096);
        let new_phys = physical_allocator::allocate_contiguous(pages);
        let old_ptr = translate_phys_virt_addr(self.data).0 as *const u8;
        let new_ptr = translate_phys_virt_addr(new_phys).0 as *mut u8;
        unsafe { core::ptr::copy_nonoverlapping(old_ptr, new_ptr, self.length as usize) };
        Self {
            data: new_phys,
            length: self.length,
        }
    }
}
