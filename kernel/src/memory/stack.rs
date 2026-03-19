use std::mem_utils::VirtAddr;

use super::PAGE_TREE_ALLOCATOR;

pub const KERNEL_STACK_SIZE_PAGES: u8 = 16;

///Create a stack with appropriate permissions and return the new stack pointer.
///Pushes an illegal return address of 0 (and aligns to 16)
pub fn prepare_kernel_stack(stack_size_pages: u8) -> VirtAddr {
    unsafe {
        let addr = PAGE_TREE_ALLOCATOR.allocate_contigious(stack_size_pages as u64 + 1, None, false);
        let lowest_entry = PAGE_TREE_ALLOCATOR
            .get_page_table_entry_mut(addr)
            .expect("page entry must exist after allocation");
        lowest_entry.set_writeable(false);
        let highest_entry = PAGE_TREE_ALLOCATOR
            .get_page_table_entry_mut(addr + (stack_size_pages as u64) * 0x1000)
            .expect("page entry must exist after allocation");
        let highest_phys_addr = highest_entry.address();
        for i in (highest_phys_addr.0 - 16)..highest_phys_addr.0 {
            let byte_ptr = i as *mut u8;
            byte_ptr.write_volatile(0);
        }

        addr + (stack_size_pages as u64 + 1) * 0x1000 - 16
    }
}
