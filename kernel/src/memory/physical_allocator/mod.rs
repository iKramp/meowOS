use std::mem_utils::PhysAddr;

mod buddy_allocator;
mod simple_phys_allocator;

// use buddy_allocator as phys_allocator;
use simple_phys_allocator as phys_allocator;

trait PhysicalAllocator {
    fn allocate(&mut self) -> PhysAddr;
    fn allocate_contiguous(&mut self, n_pages: u32) -> PhysAddr;
    fn deallocate(&mut self, addr: PhysAddr);
    fn deallocate_contiguous(&mut self, addr: PhysAddr, n_pages: u32);
    fn reserve_low(&mut self) -> PhysAddr;
}

pub fn allocate_frame() -> PhysAddr {
    phys_allocator::allocate_frame()
}

pub unsafe fn deallocate_frame(addr: PhysAddr) {
    unsafe { phys_allocator::deallocate_frame(addr) };
}

pub fn allocate_contiguous(n_pages: u32) -> PhysAddr {
    phys_allocator::allocate_contiguous(n_pages)
}

pub(super) fn init() {
    let memory_regions = unsafe { &mut *(*crate::LIMINE_BOOTLOADER_REQUESTS.memory_map_request.info).memory_map };
    let memory_regions = unsafe {
        core::slice::from_raw_parts_mut(
            memory_regions,
            (*crate::LIMINE_BOOTLOADER_REQUESTS.memory_map_request.info).memory_map_count as usize,
        )
    };

    phys_allocator::init(memory_regions);
}

pub(super) fn reserve_low() -> PhysAddr {
    phys_allocator::reserve_low()
}
