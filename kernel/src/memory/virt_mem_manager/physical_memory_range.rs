use std::mem_utils::PhysAddr;

use crate::memory::physical_allocator;

#[derive(Debug)]
pub(super) struct PhysicalMmeoryRange {
    addr: PhysAddr,
    len_frames: u64,
}

impl Drop for PhysicalMmeoryRange {
    fn drop(&mut self) {
        for i in 0..self.len_frames {
            let addr = self.addr + 4096 * i;
            unsafe { physical_allocator::deallocate_frame(addr) };
        }
    }
}
