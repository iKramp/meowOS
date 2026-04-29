use core::{mem::MaybeUninit, ops::Range};
use std::{
    mem_utils::{PhysAddr, VirtAddr, get_at_physical_addr, memset_physical_addr, set_static_lifetime_mut},
    println,
    vec::Vec,
};

use crate::memory::{
    VirtualMemoryRangePermissions, physical_allocator,
    virt_mem_manager::{flush_tlb, page_table_entry::PageTableEntry},
};

#[repr(C)]
pub(super) struct PageTable {
    pub(super) entries: [PageTableEntry; 512],
}

const _: () = {
    assert!(core::mem::size_of::<PageTable>() == 4096);
};

impl PageTable {
    pub fn clear(&mut self) {
        let default_entry = PageTableEntry::blank();
        self.entries = [default_entry; 512];
    }

    ///Deallocates the physical memory taken up by the page tree
    ///Optionally deallocates the physical memory this tree eventually points to
    ///Level 1 is the lowest valid level
    pub fn delete(self_phys: PhysAddr, level: u8, dealloc_phys: bool) {
        if level != 1 || dealloc_phys {
            let self_obj = unsafe { get_at_physical_addr::<Self>(self_phys) };

            for entry in self_obj.entries {
                if !entry.present() {
                    continue;
                }
                let phys_addr = entry.address();

                if level == 1 {
                    if dealloc_phys {
                        unsafe { physical_allocator::deallocate_frame(phys_addr) };
                    }
                    continue;
                }

                if entry.huge_page() {
                    if dealloc_phys {
                        let size_pages = 512_u64.pow(level as u32 - 1);
                        for i in 0..size_pages {
                            let addr_to_dealloc = phys_addr + i * 4096;
                            unsafe { physical_allocator::deallocate_frame(addr_to_dealloc) };
                        }
                    }
                    continue;
                }

                Self::delete(phys_addr, level - 1, dealloc_phys);
            }
        }

        unsafe { physical_allocator::deallocate_frame(self_phys) };
    }

    /// Intended to be used for MMIO, or physical ram in very rare cases.
    /// Caller must ensure the phys_addr is valid and owned
    /// Retursns the first page table entry, useful when mapping a single page
    pub unsafe fn kernel_manual_map(
        &mut self,
        phys_addr: PhysAddr,
        virt_addr: VirtAddr,
        n_pages: u64,
        current_node_virt: VirtAddr,
        current_node_level: u8,
    ) -> (&'static mut PageTableEntry, u64) {
        assert!(n_pages > 0);

        let addr_diff = virt_addr - current_node_virt;
        let start_index = (addr_diff.0 >> (12 + 9 * (current_node_level - 1))) & 0x1FF;

        if current_node_level == 1 {
            let max_index = (start_index + n_pages).min(self.entries.len() as u64);

            for i in start_index..max_index {
                let entry = &mut self.entries[i as usize];

                if entry.present() {
                    panic!("mapping already mapped virtual area");
                }

                let addr_offset = (i - start_index) * 0x1000;
                *entry = PageTableEntry::new(phys_addr + addr_offset, false);
                flush_tlb(Some(virt_addr + addr_offset))
            }

            return (
                unsafe { set_static_lifetime_mut(&mut self.entries[start_index as usize]) },
                max_index - start_index,
            );
        }

        let mut index = start_index;
        let mut allocated = 0;
        let mut first_entry = MaybeUninit::uninit();
        while allocated < n_pages && index < self.entries.len() as u64 {
            let entry = &mut self.entries[index as usize];

            let lower_table_phys;
            if !entry.present() {
                let new_frame = physical_allocator::allocate_frame();
                unsafe { memset_physical_addr(new_frame, 0, 4096) };
                *entry = PageTableEntry::new(new_frame, false);
                lower_table_phys = new_frame;
            } else {
                lower_table_phys = entry.address();
            }

            if entry.huge_page() {
                panic!("not dealing with huge pages");
            }

            let new_table = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
            let new_node_virt = current_node_virt + (index << (12 + 9 * (current_node_level - 1)));

            let left_to_alloc = n_pages - allocated;

            let (first_entry_, newly_allocated) = unsafe {
                new_table.kernel_manual_map(
                    phys_addr + allocated * 0x1000,
                    virt_addr + allocated * 0x1000,
                    left_to_alloc,
                    new_node_virt,
                    current_node_level - 1,
                )
            };
            allocated += newly_allocated;

            if index == start_index {
                first_entry.write(first_entry_);
            }

            index += 1;
        }
        (unsafe { first_entry.assume_init() }, allocated)
    }

    /// Intended to be used for MMIO, or physical ram in very rare cases.
    /// Caller must release the physical memory
    pub unsafe fn kernel_manual_unmap(
        &mut self,
        virt_addr: VirtAddr,
        n_pages: u64,
        current_node_virt: VirtAddr,
        current_node_level: u8,
    ) -> u64 {
        assert!(n_pages > 0);
        let addr_diff = virt_addr - current_node_virt;
        let start_index = (addr_diff.0 >> (12 + 9 * (current_node_level - 1))) & 0x1FF;

        if current_node_level == 1 {
            let max_index = (start_index + n_pages).min(self.entries.len() as u64);
            for i in start_index..max_index {
                let entry = &mut self.entries[i as usize];

                if !entry.present() {
                    panic!("unmapping already unmapped area")
                }

                let addr_offset = (i - start_index) * 0x1000;
                entry.set_present(false);
                flush_tlb(Some(virt_addr + addr_offset));
            }

            return max_index - start_index;
        }

        let mut index = start_index;
        let mut freed = 0;
        while freed < n_pages && index < self.entries.len() as u64 {
            let entry = &mut self.entries[index as usize];

            if !entry.present() {
                panic!("unmapping unmapped area");
            }
            if entry.huge_page() {
                panic!("not dealing with huge pages");
            }

            let lower_table_phys = entry.address();
            let lower_table = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
            let new_node_virt = current_node_virt + (index << (12 + 9 * (current_node_level - 1)));

            let left_to_alloc = n_pages - freed;

            let newly_freed = unsafe {
                lower_table.kernel_manual_unmap(
                    virt_addr + freed * 0x1000,
                    left_to_alloc,
                    new_node_virt,
                    current_node_level - 1,
                )
            };

            if lower_table.entries.iter().all(|e| !e.present()) {
                entry.set_present(false);
                unsafe { physical_allocator::deallocate_frame(lower_table_phys) };
            }

            freed += newly_freed;
            index += 1;
        }
        freed
    }

    pub fn userspace_map(
        &mut self,
        page_range: Range<u32>,
        permissions: VirtualMemoryRangePermissions,
        table_level: u8,
        table_page_index: u32,
    ) {
        if page_range.is_empty() {
            return;
        }
        let pages_per_entry = 512_u64.pow(table_level as u32 - 1) as u32;

        let start_page = page_range.start.max(table_page_index);
        let end_page = page_range.end.min(table_page_index + pages_per_entry * 512);

        let start_index = ((start_page - table_page_index) / pages_per_entry) as usize;
        let end_index = (end_page - table_page_index).div_ceil(pages_per_entry) as usize;

        for i in start_index..end_index {
            let entry = &mut self.entries[i];

            if table_level == 1 {
                if entry.present() {
                    continue;
                }
                let entry_phys = physical_allocator::allocate_frame();
                *entry = PageTableEntry::new(entry_phys, true);
                entry.set_writeable(permissions.write());
                entry.set_no_execute(!permissions.execute());
                println!("allocated userspace page: {:X?}", entry);
                continue;
            }

            //not lowest level
            if !entry.present() {
                let new_frame = physical_allocator::allocate_frame();
                unsafe { memset_physical_addr(new_frame, 0, 4096) };
                *entry = PageTableEntry::new(new_frame, true);
            } else if entry.huge_page() {
                panic!("no huge pages in userspace for now")
            }

            entry.set_writeable(entry.writeable() || permissions.write());
            entry.set_no_execute(entry.no_execute() && !permissions.execute());

            let lower_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
            let lower_table_page_index = table_page_index + (i as u32) * pages_per_entry;
            lower_table.userspace_map(page_range.clone(), permissions, table_level - 1, lower_table_page_index);
        }
    }

    pub fn userspace_unmap(&mut self, page_range: Range<u32>, table_level: u8, table_page_index: u32) {
        if page_range.is_empty() {
            return;
        }
        let pages_per_entry = 512_u64.pow(table_level as u32 - 1) as u32;

        let start_page = page_range.start.max(table_page_index);
        let end_page = page_range.end.min(table_page_index + pages_per_entry * 512);
        let start_index = ((start_page - table_page_index) / pages_per_entry) as usize;
        let end_index = (end_page - table_page_index).div_ceil(pages_per_entry) as usize;
        let mut freed_phys = Vec::new();

        for i in start_index..end_index {
            let entry = &mut self.entries[i];

            if !entry.present() {
                continue;
            }

            if table_level == 1 {
                freed_phys.push(entry.address());
                *entry = PageTableEntry::blank();
                continue;
            }

            //not lowest level
            if entry.huge_page() {
                panic!("no huge pages in userspace for now")
            }

            let lower_table_phys = entry.address();
            let lower_table = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
            let lower_table_page_index = table_page_index + (i as u32) * pages_per_entry;
            lower_table.userspace_unmap(page_range.clone(), table_level - 1, lower_table_page_index);

            if lower_table.entries.iter().all(|e| !e.present()) {
                *entry = PageTableEntry::blank();
                unsafe { physical_allocator::deallocate_frame(lower_table_phys) };
            }
        }
    }

    pub fn set_prot(
        &mut self,
        addr_range: Range<VirtAddr>,
        permissions: VirtualMemoryRangePermissions,
        table_level: u8,
        table_addr: VirtAddr,
    ) {
        if addr_range.is_empty() {
            return;
        }
        let pages_per_entry = 512_u64.pow(table_level as u32 - 1) as u32;
        println!("pages per entry at level {}: {}", table_level, pages_per_entry);

        let start_addr = addr_range.start.max(table_addr);
        let end_addr = addr_range.end.min(table_addr + pages_per_entry as u64 * 512 * 4096);
        let start_index = (start_addr - table_addr).0 / (pages_per_entry as u64 * 4096);
        let end_index = (end_addr - table_addr).0.div_ceil(pages_per_entry as u64 * 4096);
        println!(
            "setting prot for addr range {:?} at level {}, start index: {}, end index: {}",
            addr_range, table_level, start_index, end_index
        );
        println!("start addr: {:?}, end addr: {:?}", start_addr, end_addr);

        for i in start_index..end_index {
            let entry = &mut self.entries[i as usize];

            if !entry.present() {
                println!("entry not present at level {}, index {}, skipping", table_level, i);
                continue;
            }

            println!(
                "setting prot at level {} for addr_range: {:?}, permissions: {:?}",
                table_level, addr_range, permissions
            );
            if table_level == 1 || entry.huge_page() {
                println!("setting prot at lowest level or huge page, entry before: {:?}", entry);
                entry.set_writeable(permissions.write());
                entry.set_no_execute(!permissions.execute());
                flush_tlb(Some(table_addr + i * pages_per_entry as u64 * 4096));
                continue;
            }

            entry.set_writeable(entry.writeable() || permissions.write());
            entry.set_no_execute(entry.no_execute() && !permissions.execute());

            println!("setting prot at level {}, entry after: {:?}", table_level, entry);

            let lower_table_phys = entry.address();
            let lower_table = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
            let lower_table_addr = table_addr + i * pages_per_entry as u64 * 4096;
            println!(
                "recursively setting prot for lower table at level {}, page addr: {:?}",
                table_level - 1,
                lower_table_addr
            );
            lower_table.set_prot(addr_range.clone(), permissions, table_level - 1, lower_table_addr);
        }
    }

    pub fn get_page_table_entry(
        &mut self,
        virt_addr: VirtAddr,
        current_node_virt: VirtAddr,
        current_node_level: u8,
        desired_level: u8,
        allocate_missing: bool,
    ) -> Option<&'static mut PageTableEntry> {
        let addr_diff = virt_addr - current_node_virt;
        let index = (addr_diff.0 >> (12 + 9 * (current_node_level - 1))) & 0x1FF;
        let entry = &mut self.entries[index as usize];

        if current_node_level == desired_level {
            return unsafe { Some(set_static_lifetime_mut(entry)) };
        }

        if !entry.present() {
            if allocate_missing {
                let new_frame = physical_allocator::allocate_frame();
                unsafe { memset_physical_addr(new_frame, 0, 4096) };
                let user_mode = virt_addr.0 < (1 << 48);
                *entry = PageTableEntry::new(new_frame, user_mode);
                entry.set_writeable(true);
                entry.set_no_execute(false); //permissions on lower levels
            } else {
                return None;
            }
        }

        if entry.huge_page() {
            println!("huge page entry while getting entry at level");
            return None;
        }

        let lower_table_phys = entry.address();
        let new_table = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
        let new_node_virt = current_node_virt + (index << (12 + 9 * (current_node_level - 1)));

        new_table.get_page_table_entry(
            virt_addr,
            new_node_virt,
            current_node_level - 1,
            desired_level,
            allocate_missing,
        )
    }

    pub fn get_table_at_level(
        &self,
        addr: VirtAddr,
        current_node_virt: VirtAddr,
        current_node_level: u8,
        wanted_node_level: u8,
    ) -> Option<&'static mut PageTable> {
        let addr_diff = addr - current_node_virt;
        let index = (addr_diff.0 >> (12 + 9 * (current_node_level - 1))) & 0x1FF;
        let entry = &self.entries[index as usize];

        if !entry.present() {
            return None;
        }

        if current_node_level == 1 {
            panic!("level 1 page tables don't point to other tables");
        }

        if entry.huge_page() {
            return None;
        }

        let lower_table_phys = entry.address();
        let new_node_virt = current_node_virt + (index << (12 + 9 * (current_node_level - 1)));
        let new_node = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
        if current_node_level == wanted_node_level + 1 {
            Some(new_node)
        } else {
            new_node.get_table_at_level(addr, new_node_virt, current_node_level - 1, wanted_node_level)
        }
    }

    pub fn get_free_ranges(&mut self, current_node_virt: VirtAddr, current_node_level: u8) -> Vec<(VirtAddr, u64)> {
        let mut res = Vec::new();
        let mut curr_range = None;

        for (index, entry) in self.entries.iter().enumerate() {
            let addr = current_node_virt + ((index as u64) << (12 + 9 * (current_node_level as u64 - 1)));

            if !entry.present() {
                let mut curr = curr_range.take().unwrap_or((addr, 0));
                let pages = 1 << ((current_node_level - 1) * 9);
                curr.1 += pages;
                curr_range = Some(curr);
                continue;
            }

            //entry is present
            if entry.huge_page() || current_node_level == 1 {
                if let Some(curr) = curr_range.take() {
                    res.push(curr);
                }
                continue;
            }

            //entry is present and has lower node
            let lower_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
            let mut lower_ranges = lower_table.get_free_ranges(addr, current_node_level - 1);
            if let Some(range) = &curr_range
                && let Some(first) = lower_ranges.first()
                && range.0 + range.1 * 0x1000 == first.0
            {
                let mut curr = unsafe { curr_range.take().unwrap_unchecked() };
                curr.1 += first.1;
                lower_ranges[0] = curr;
            }
            if let Some(curr) = curr_range.take() {
                res.push(curr);
            }
            curr_range = lower_ranges.pop();
            res.extend(lower_ranges);
        }

        if let Some(curr) = curr_range {
            res.push(curr);
        }

        res
    }
}
