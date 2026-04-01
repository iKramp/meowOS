use bitfield::bitfield;
use core::ops::Range;
use std::{
    mem_utils::{PhysAddr, VirtAddr, get_at_physical_addr},
    vec::Vec,
};

use crate::memory::{
    physical_allocator,
    virt_mem_manager::{page_table::PageTable, physical_memory_range::PhysicalMmeoryRange},
};

#[derive(Debug, Clone, Copy)]
pub enum VirtualMemoryRangeCapacity {
    _4KB,
    _2MB,
    _1GB,
    _05TB,
}

impl VirtualMemoryRangeCapacity {
    pub fn from_level(level: u8) -> Self {
        match level {
            0 => VirtualMemoryRangeCapacity::_4KB,
            1 => VirtualMemoryRangeCapacity::_2MB,
            2 => VirtualMemoryRangeCapacity::_1GB,
            3 => VirtualMemoryRangeCapacity::_05TB,
            _ => panic!("invalid value in virtual memory range level"),
        }
    }

    pub fn into_level(self) -> u8 {
        match self {
            VirtualMemoryRangeCapacity::_4KB => 0,
            VirtualMemoryRangeCapacity::_2MB => 1,
            VirtualMemoryRangeCapacity::_1GB => 2,
            VirtualMemoryRangeCapacity::_05TB => 3,
        }
    }

    pub fn reserved_range(&self, start: VirtAddr) -> Range<VirtAddr> {
        let level = self.clone().into_level();
        let size = 4096 * 512u64.pow(level as u32);
        start..(start + size)
    }

    pub fn align_up(&self, addr: VirtAddr) -> VirtAddr {
        let level = self.clone().into_level();
        let size = 4096 * 512u64.pow(level as u32);
        VirtAddr((addr.0 + size - 1) & !(size - 1))
    }

    pub fn align_down(&self, addr: VirtAddr) -> VirtAddr {
        let level = self.clone().into_level();
        let size = 4096 * 512u64.pow(level as u32);
        VirtAddr(addr.0 & !(size - 1))
    }
}

bitfield! {
    pub struct VirtualMemoryRangePermissions(u8);
    impl Debug;
    pub write, set_write: 0;
    pub execute, set_execute: 1;
}

#[derive(Debug)]
pub struct VirtualMemoryRange {
    phys_ranges: Vec<PhysicalMmeoryRange>,
    virt_tree_node: PhysAddr,
    virt_tree_level: u8, //0 means just 1 page, 1 means page tree node with allocated pages below
    allocated_pages: u64,
    perms: VirtualMemoryRangePermissions,
}

impl VirtualMemoryRange {
    pub fn max_size(&self) -> VirtualMemoryRangeCapacity {
        VirtualMemoryRangeCapacity::from_level(self.virt_tree_level)
    }

    pub fn current_size_pages(&self) -> u64 {
        self.allocated_pages
    }

    pub fn level(&self) -> u8 {
        self.virt_tree_level
    }

    pub fn reserved_range(&self, start_addr: VirtAddr) -> Range<VirtAddr> {
        self.max_size().reserved_range(start_addr)
    }

    pub fn node_addr(&self) -> PhysAddr {
        self.virt_tree_node
    }

    pub fn permissions(&self) -> VirtualMemoryRangePermissions {
        self.perms
    }

    pub fn create(capacity: VirtualMemoryRangeCapacity, perms: VirtualMemoryRangePermissions) -> Self {
        let table_addr = physical_allocator::allocate_frame();
        let table = unsafe { get_at_physical_addr::<PageTable>(table_addr) };
        table.clear();

        Self {
            phys_ranges: Vec::new(),
            virt_tree_node: table_addr,
            virt_tree_level: capacity.into_level(),
            allocated_pages: 0,
            perms,
        }
    }

    pub fn expand_by(&mut self, n_pages: u64) {
        let current_pages = self.allocated_pages;
        let new_size = current_pages.saturating_add(n_pages);
        self.expand_to(new_size);
    }

    pub fn expand_to(&mut self, _n_pages: u64) {
        todo!("expand userspace memory range")
    }

    pub fn shrink_by(&mut self, n_pages: u64) {
        let current_pages = self.allocated_pages;
        let new_size = current_pages.saturating_sub(n_pages);
        self.shrink_to(new_size);
    }

    pub fn shrink_to(&mut self, _n_pages: u64) {
        todo!("shrink userspace memory range")
    }
}

impl Drop for VirtualMemoryRange {
    fn drop(&mut self) {
        //physical deallocated by dropping the phys_ranges
        PageTable::delete(self.virt_tree_node, self.virt_tree_level, false);
    }
}
