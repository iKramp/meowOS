use crate::memory::{self, addresses::VirtAddr, physical_allocator};

pub const KERNEL_STACK_SIZE_PAGES: u8 = 16;

///Create a stack with appropriate permissions and return the new stack pointer.
///Pushes an illegal return address of 0 (and aligns to 16)
pub fn prepare_kernel_stack(stack_size_pages: u8) -> VirtAddr {
    unsafe {
        let phys_addr = physical_allocator::allocate_contiguous(stack_size_pages as u32 + 1);
        let (addr, lowest_entry) = memory::kernel_manual_map(phys_addr, stack_size_pages as u64 + 1, None);
        lowest_entry.set_writeable(false);
        let highest_addr = addr + (stack_size_pages as u64 + 1) * 0x1000;
        for i in (highest_addr.0 - 16)..highest_addr.0 {
            let byte_ptr = i as *mut u8;
            byte_ptr.write(0);
        }

        highest_addr - 16_u64
    }
}
