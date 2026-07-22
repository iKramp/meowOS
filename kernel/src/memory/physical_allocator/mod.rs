use std::println;

use crate::{limine, memory::addresses::*};
mod buddy_allocator;
mod simple_phys_allocator;

// use buddy_allocator as phys_allocator;
use simple_phys_allocator as phys_allocator;

trait PhysicalAllocator {
    fn allocate(&mut self) -> OwnedPhysAddr;
    fn allocate_contiguous(&mut self, n_pages: u32) -> OwnedPhysRange;
    fn deallocate<T: OwnedPhysicalRangeData>(&mut self, addr: &T);
    fn reserve_low(&mut self) -> OwnedPhysAddr;
}

pub static mut MAX_RAM_ADDR: PhysAddr = PhysAddr(0);

pub fn is_on_ram(addr: PhysAddr) -> bool {
    addr.0 <= unsafe { MAX_RAM_ADDR.0 }
}

pub fn allocate() -> OwnedPhysAddr {
    phys_allocator::allocate_frame()
}

pub unsafe fn deallocate<T: Into<OwnedPhysRange>>(addr: T) {
    let addr: OwnedPhysRange = addr.into();
    drop(addr); //techinaclly unneeded but more explicit
}

/// Safety:
/// This should not be called manually, it's used in the drop implementation
pub unsafe fn _deallocate_by_ref<T: OwnedPhysicalRangeData>(addr: &T) {
    if addr.get_range().n_pages == 0 || addr.get_range().start.0 == 0 {
        return;
    }
    if !is_on_ram(addr.get_range().start) {
        return;
    }
    unsafe { phys_allocator::deallocate(addr) };
}

impl Drop for OwnedPhysAddr {
    fn drop(&mut self) {
        let range = PhysRange {
            start: self.0,
            n_pages: 1,
        };
        let owned_range = OwnedPhysRange(range);
        drop(owned_range);
    }
}

pub fn allocate_contiguous(n_pages: u32) -> OwnedPhysRange {
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

    let n_pages = find_max_ram_address(memory_regions).0 >> 12;
    unsafe { MAX_RAM_ADDR = PhysAddr(n_pages << 12) };
    println!(level:info, "n_pages: {}", n_pages);
    println!(level:info, "max memory address: {:#X}", n_pages * 4096);

    phys_allocator::init(memory_regions);
}

pub(super) fn reserve_low() -> OwnedPhysAddr {
    phys_allocator::reserve_low()
}

fn find_max_ram_address(memory_regions: &[&mut limine::MemoryMapEntry]) -> PhysAddr {
    let mut highest = 0;
    for region in memory_regions {
        if region.can_be_usable() {
            highest = region.base + region.length;
        }
    }
    PhysAddr(highest)
}
