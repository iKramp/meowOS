use std::mem_utils::{PhysAddr, get_at_physical_addr};

use crate::memory::{virt_mem_manager::page_table_entry::PageTableEntry, physical_allocator};

#[repr(C)]
pub(super) struct PageTable {
    entries: [PageTableEntry; 512],
}

const _: () = {
    assert!(core::mem::size_of::<PageTable>() == 4096);
};

impl PageTable {
    pub fn clear(&mut self) {
        let default_entry = PageTableEntry::blank();
        self.entries = [default_entry; 512];
    }

    ///Deallocates the physical memory take up by the page tree
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
}
