use std::{
    mem_utils::{PhysAddr, get_at_physical_addr},
    sync::arc::Arc,
    vec::Vec,
};

use crate::memory::{
    physical_allocator,
    virt_mem_manager::{page_table::PageTable, physical_memory_range::PhysicalMmeoryRange},
};

enum VirtualMemoryRangeCapacity {
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
}

struct VirtualMemoryRange {
    phys_ranges: Vec<Arc<PhysicalMmeoryRange>>,
    virt_tree_node: PhysAddr,
    virt_tree_level: u8, //0 means just 1 page, 1 means page tree node with allocated pages below
    allocated_pages: u64,
}

impl VirtualMemoryRange {
    pub fn max_size(&self) -> VirtualMemoryRangeCapacity {
        VirtualMemoryRangeCapacity::from_level(self.virt_tree_level)
    }

    pub fn create(capacity: VirtualMemoryRangeCapacity) -> Self {
        let table_addr = physical_allocator::allocate_frame();
        let table = unsafe { get_at_physical_addr::<PageTable>(table_addr) };
        table.clear();

        Self {
            phys_ranges: Vec::new(),
            virt_tree_node: table_addr,
            virt_tree_level: capacity.into_level(),
            allocated_pages: 0,
        }
    }

    pub fn expand_by(&mut self, n_pages: u64) {
        let current_pages = self.allocated_pages;
        let new_size = current_pages.saturating_add(n_pages);
        self.expand_to(new_size);
    }

    pub fn expand_to(&mut self, n_pages: u64) {
        todo!()
    }

    pub fn shrink_by(&mut self, n_pages: u64) {
        let current_pages = self.allocated_pages;
        let new_size = current_pages.saturating_sub(n_pages);
        self.shrink_to(new_size);
    }

    pub fn shrink_to(&mut self, n_pages: u64) {
        todo!()
    }
}

impl Drop for VirtualMemoryRange {
    fn drop(&mut self) {
        //physical deallocated by dropping the phys_ranges
        PageTable::delete(self.virt_tree_node, self.virt_tree_level, false);
    }
}
