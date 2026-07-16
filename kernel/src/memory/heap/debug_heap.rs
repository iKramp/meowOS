use std::mem_utils::PhysAddr;

use crate::memory::physical_allocator;

struct DebugHeap;

#[global_allocator]
static HEAP: DebugHeap = DebugHeap;

unsafe impl core::alloc::GlobalAlloc for DebugHeap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let size_pages = layout.size().div_ceil(4096);
        physical_allocator::allocate_contiguous(size_pages as u32).0 as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let size_pages = layout.size().div_ceil(4096);
        for page in 0..size_pages {
            let addr = PhysAddr((ptr as u64) + (page as u64 * 4096));
            unsafe { physical_allocator::deallocate_frame(addr) };
        }
    }
}
