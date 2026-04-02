use std::{
    boxed::Box,
    error::ErrorCode,
    mem_utils::{PhysAddr, VirtAddr},
    println,
    sync::arc::Arc,
    vec::Vec,
};

use crate::{
    memory::{self, PageTableEntry, VirtualMemoryRange, VirtualMemoryRangeCapacity},
    proc::namespaces::ProcNamespace,
};

#[derive(Debug, PartialEq, Eq)]
pub(in crate::proc) enum MemoryRangeType {
    Stack,
    Code,
    Data,
    Shared,
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
    page_tree_root: PhysAddr,
    range_counter: u32,
    memory_ranges: Vec<OwnedVirtualMemoryRange>,
}
impl ProcNamespace for MemoryNamespace {}

impl MemoryNamespace {
    pub fn new(page_tree_root: PhysAddr) -> Self {
        Self {
            page_tree_root,
            memory_ranges: Vec::new(),
            range_counter: 0,
        }
    }

    pub fn page_tree_root(&self) -> PhysAddr {
        self.page_tree_root
    }

    pub fn add_mem_range(
        &mut self,
        range: Arc<VirtualMemoryRange>,
        name: Box<str>,
        range_type: MemoryRangeType,
        map_address: VirtAddr,
    ) -> Result<(), ErrorCode> {
        let new_range = range.reserved_range(map_address);
        if new_range.end.0 >= (1 << 48) {
            //disallow mapping in kernel space
            return Err(ErrorCode::InvalidArgument);
        }

        let illegal_map = self.memory_ranges.iter().any(|r| {
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
            println!("checks did not catch invalid map");
            return Err(ErrorCode::InvalidArgument);
        }

        *entry = PageTableEntry::new(range.node_addr(), true);

        entry.set_no_execute(false);
        entry.set_writeable(true);
        //restrictions apply at lower levels

        self.memory_ranges.push(OwnedVirtualMemoryRange {
            shared_range: range,
            name,
            range_type,
            map_address,
            range_id: self.range_counter,
        });
        self.range_counter += 1;
        Ok(())
    }

    pub fn get_by_containing_addr(&self, addr: VirtAddr) -> Option<&OwnedVirtualMemoryRange> {
        self.memory_ranges.iter().find(|r| {
            let r_range = r.shared_range.reserved_range(r.map_address);
            r_range.start <= addr && addr < r_range.end
        })
    }

    pub fn remove_mem_range_by_name(&mut self, name: &str) -> Result<(), ErrorCode> {
        let index = self
            .memory_ranges
            .iter()
            .position(|r| *r.name == *name)
            .ok_or(ErrorCode::NoEntry)?;
        self.remove_mem_range_by_index(index as u32);
        Ok(())
    }

    pub fn remove_mem_range_by_id(&mut self, id: u32) -> Result<(), ErrorCode> {
        let index = self
            .memory_ranges
            .iter()
            .position(|r| r.range_id == id)
            .ok_or(ErrorCode::NoEntry)?;
        self.remove_mem_range_by_index(index as u32);
        Ok(())
    }

    pub fn remove_mem_range_by_index(&mut self, index: u32) {
        let range = self.memory_ranges.swap_remove(index as usize);
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

    pub fn find_hole(&mut self, size: VirtualMemoryRangeCapacity) -> Option<VirtAddr> {
        let mut current_addr = VirtAddr(1);
        'repeat: loop {
            current_addr = size.align_up(current_addr);
            let curr_range = size.reserved_range(current_addr);
            if curr_range.end.0 >= (1 << 48) {
                //disallow mapping in kernel space
                return None;
            }
            for range in self.memory_ranges.iter() {
                let r_range = range.shared_range.reserved_range(range.map_address);
                if r_range.start < curr_range.end && curr_range.start < r_range.end {
                    current_addr = r_range.end;
                    continue 'repeat;
                }
            }
            return Some(current_addr);
        }
    }

    pub fn regions(&self) -> &Vec<OwnedVirtualMemoryRange> {
        &self.memory_ranges
    }
}

impl Default for MemoryNamespace {
    fn default() -> Self {
        Self::new(PhysAddr(0))
    }
}

impl Drop for MemoryNamespace {
    fn drop(&mut self) {
        if self.page_tree_root.0 == 0 {
            return;
        }

        for range in self.memory_ranges.iter() {
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
