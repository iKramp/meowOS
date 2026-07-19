use std::{
    boxed::Box,
    error::ErrorCode,
    lock_w_info, println,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use crate::{
    memory::{self, PageTableEntry, VirtualMemoryRange, VirtualMemoryRangeCapacity, addresses::*, physical_allocator},
    proc::namespaces::ProcNamespace,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(in crate::proc) enum MemoryRangeType {
    Stack = 0,
    Code = 1,
    Data = 2,
    Shared = 3,
}

impl MemoryRangeType {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(MemoryRangeType::Stack),
            1 => Some(MemoryRangeType::Code),
            2 => Some(MemoryRangeType::Data),
            3 => Some(MemoryRangeType::Shared),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(in crate::proc) struct OwnedVirtualMemoryRange {
    pub shared_range: Arc<VirtualMemoryRange>,
    pub name: Box<str>,
    pub range_type: MemoryRangeType,
    pub map_address: VirtAddr,
    pub range_id: u32,
}

#[derive(Debug)]
pub(in crate::proc) struct MemoryNamespace {
    id: u64,
    page_tree_root: PhysAddr,
    pub dynamic_data: NoIntSpinlock<MemoryNamespaceDynamicData>,
}
#[derive(Debug)]
pub(in crate::proc) struct MemoryNamespaceDynamicData {
    range_counter: u32,
    memory_ranges: Vec<OwnedVirtualMemoryRange>,
}

impl ProcNamespace for MemoryNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn create_empty(id: u64) -> Result<Self, ErrorCode> {
        let owned_page_tree_root = physical_allocator::allocate();
        let page_tree_root = owned_page_tree_root.0;
        core::mem::forget(owned_page_tree_root); //dealloc handled by drop impl
        unsafe { memset_at_addr(page_tree_root, 0, 0x1000) };
        Ok(Self {
            id,
            page_tree_root,
            dynamic_data: NoIntSpinlock::new(MemoryNamespaceDynamicData {
                memory_ranges: Vec::new(),
                range_counter: 1,
            }),
        })
    }

    fn create_from(id: u64, other: &Self) -> Result<MemoryNamespace, ErrorCode> {
        let new_namespace = MemoryNamespace::create_empty(id)?;

        //drop current ranges
        let mut new_dynamic_data = lock_w_info!(new_namespace.dynamic_data);
        let other_dynamic = lock_w_info!(other.dynamic_data);
        new_dynamic_data.memory_ranges.clear();
        new_dynamic_data.range_counter = other_dynamic.range_counter;
        drop(new_dynamic_data);

        for range in other_dynamic.memory_ranges.iter() {
            let res = new_namespace.add_mem_range(
                range.shared_range.clone(),
                range.name.clone(),
                range.range_type,
                range.map_address,
            );
            res?;
        }

        Ok(new_namespace)
    }
}

impl MemoryNamespace {
    pub fn page_tree_root(&self) -> PhysAddr {
        self.page_tree_root
    }

    ///returns ID
    pub fn add_mem_range(
        &self,
        range: Arc<VirtualMemoryRange>,
        name: Box<str>,
        range_type: MemoryRangeType,
        map_address: VirtAddr,
    ) -> Result<u32, ErrorCode> {
        let new_range = range.reserved_range(map_address);
        if new_range.end.0 >= (1 << 48) {
            //disallow mapping in kernel space
            return Err(ErrorCode::InvalidArgument);
        }
        let mut dynamic = lock_w_info!(self.dynamic_data);

        let illegal_map = dynamic.memory_ranges.iter().any(|r| {
            r.name == name || {
                let r_range = r.shared_range.reserved_range(r.map_address);
                r_range.start < new_range.end && new_range.start < r_range.end
            }
        });
        if illegal_map {
            return Err(ErrorCode::InvalidArgument);
        }

        let Some(entry) = memory::get_page_table_entry_at_level(self.page_tree_root, map_address, range.level() + 1, true) else {
            println!("checks did not catch invalid map");
            return Err(ErrorCode::InvalidArgument);
        };

        if entry.present() {
            return Err(ErrorCode::InvalidArgument);
        }

        *entry = PageTableEntry::new(range.node_addr(), true);

        entry.set_no_execute(false);
        entry.set_writeable(true);
        //restrictions apply at lower levels

        let counter = dynamic.range_counter;
        dynamic.memory_ranges.push(OwnedVirtualMemoryRange {
            shared_range: range,
            name,
            range_type,
            map_address,
            range_id: counter,
        });
        dynamic.range_counter += 1;
        Ok(counter)
    }

    pub fn remove_mem_range_by_name(&self, name: &str) -> Result<(), ErrorCode> {
        let mut dynamic_data = lock_w_info!(self.dynamic_data);
        let index = dynamic_data
            .memory_ranges
            .iter()
            .position(|r| *r.name == *name)
            .ok_or(ErrorCode::NoEntry)?;
        self.remove_mem_range_by_index(&mut dynamic_data, index as u32);
        Ok(())
    }

    pub fn remove_mem_range_by_id(&self, id: u32) -> Result<(), ErrorCode> {
        let mut dynamic_data = lock_w_info!(self.dynamic_data);
        let index = dynamic_data
            .memory_ranges
            .iter()
            .position(|r| r.range_id == id)
            .ok_or(ErrorCode::NoEntry)?;
        self.remove_mem_range_by_index(&mut dynamic_data, index as u32);
        Ok(())
    }

    //pass in locked data so it can't be modified between finding the index and removing the range
    fn remove_mem_range_by_index(&self, dynamic_data: &mut MemoryNamespaceDynamicData, index: u32) {
        let range = dynamic_data.memory_ranges.swap_remove(index as usize);
        let Some(table_entry) =
            memory::get_page_table_entry_at_level(self.page_tree_root, range.map_address, range.shared_range.level() + 1, false)
        else {
            println!("checks did not catch invalid unmap");
            return;
        };
        if table_entry.address() != range.shared_range.node_addr() {
            println!("checks did not catch invalid unmap");
            return;
        }

        *table_entry = PageTableEntry(0);

        let current_root = memory::current_root();
        if current_root == self.page_tree_root {
            // a single flush is enough, because it flushes all levels leading to this addr
            // This flushes the "root" too, which is now set to not present. For any address in
            // this range, the next access will load the root and find the entry not present
            memory::flush_tlb(Some(range.map_address));
        }
    }

    pub fn find_hole(&self, size: VirtualMemoryRangeCapacity) -> Option<VirtAddr> {
        let mut current_addr = VirtAddr(1);
        'repeat: loop {
            current_addr = size.align_up(current_addr);
            let curr_range = size.reserved_range(current_addr);
            if curr_range.end.0 >= (1 << 48) {
                //disallow mapping in kernel space
                return None;
            }
            for range in lock_w_info!(self.dynamic_data).memory_ranges.iter() {
                let r_range = range.shared_range.reserved_range(range.map_address);
                if r_range.start < curr_range.end && curr_range.start < r_range.end {
                    current_addr = r_range.end;
                    continue 'repeat;
                }
            }
            return Some(current_addr);
        }
    }

    //returns (range, base address)
    pub fn get_range_from_address(&self, addr: VirtAddr) -> Option<(Arc<VirtualMemoryRange>, VirtAddr)> {
        for range in lock_w_info!(self.dynamic_data).memory_ranges.iter() {
            let r_range = range.shared_range.reserved_range(range.map_address);
            if r_range.start <= addr && addr < r_range.end {
                return Some((range.shared_range.clone(), range.map_address));
            }
        }
        None
    }

    pub fn get_range_from_id(&self, id: u32) -> Option<(Arc<VirtualMemoryRange>, VirtAddr)> {
        for range in lock_w_info!(self.dynamic_data).memory_ranges.iter() {
            if range.range_id == id {
                return Some((range.shared_range.clone(), range.map_address));
            }
        }
        None
    }
}

impl MemoryNamespaceDynamicData {
    pub fn regions(&self) -> &Vec<OwnedVirtualMemoryRange> {
        &self.memory_ranges
    }
}

impl Drop for MemoryNamespace {
    fn drop(&mut self) {
        for range in lock_w_info!(self.dynamic_data).memory_ranges.iter() {
            let Some(table_entry) = memory::get_page_table_entry_at_level(
                self.page_tree_root,
                range.map_address,
                range.shared_range.level() + 1,
                false,
            ) else {
                println!("checks did not catch invalid unmap");
                continue;
            };
            if table_entry.address() != range.shared_range.node_addr() {
                println!("checks did not catch invalid unmap");
                continue;
            }

            table_entry.0 = 0;
        }

        memory::delete_page_table(self.page_tree_root, 4, false);
    }
}
