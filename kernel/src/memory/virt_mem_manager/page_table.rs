use core::mem::MaybeUninit;
use std::{
    mem_utils::{PhysAddr, VirtAddr, get_at_physical_addr, memset_physical_addr, set_static_lifetime_mut},
    vec::Vec,
};

use crate::memory::{
    physical_allocator,
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
                let phys_addr = entry.address();

                if level == 1 {
                    unsafe { physical_allocator::deallocate_frame(phys_addr) };
                    continue;
                }

                if entry.huge_page() && dealloc_phys {
                    let size_pages = 512_u64.pow(level as u32 - 1);
                    for i in 0..size_pages {
                        let addr_to_dealloc = phys_addr + i * 4096;
                        unsafe { physical_allocator::deallocate_frame(addr_to_dealloc) };
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

    pub fn get_page_table_entry(
        &mut self,
        virt_addr: VirtAddr,
        current_node_virt: VirtAddr,
        current_node_level: u8,
    ) -> Option<&'static mut PageTableEntry> {
        let addr_diff = virt_addr - current_node_virt;
        let index = (addr_diff.0 >> (12 + 9 * (current_node_level - 1))) & 0x1FF;
        let entry = &mut self.entries[index as usize];

        if !entry.present() {
            return None;
        }

        if current_node_level == 1 {
            return unsafe { Some(set_static_lifetime_mut(entry)) };
        }

        if entry.huge_page() {
            panic!("disallowing editing of huge page entries");
        }

        let lower_table_phys = entry.address();
        let new_table = unsafe { get_at_physical_addr::<PageTable>(lower_table_phys) };
        let new_node_virt = current_node_virt + (index << (12 + 9 * (current_node_level - 1)));

        new_table.get_page_table_entry(virt_addr, new_node_virt, current_node_level - 1)
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
