use super::PhysicalAllocator;
use std::{lock_w_info, println, sync::no_int_spinlock::NoIntSpinlock};

use crate::{limine, memory::addresses::*};

#[derive(Debug, Clone)]
#[repr(C)]
struct RegionMetadata {
    size_pages: u64,
    next: PhysAddr,
}

struct SimplePhysicalAllocator {
    start_region: PhysAddr,
}

static SIMPLE_PHYS_ALLOCATOR: NoIntSpinlock<SimplePhysicalAllocator> = NoIntSpinlock::new(SimplePhysicalAllocator {
    start_region: PhysAddr(0),
});

impl SimplePhysicalAllocator {
    fn deallocate_single(&mut self, addr: PhysAddr) {
        let mut current_base = self.start_region;
        let (next_base, current_metadata) = loop {
            let current_metadata = unsafe { get_at_addr::<RegionMetadata, _>(current_base) };
            let next_base = current_metadata.next;

            if next_base.0 != 0 && next_base.0 < addr.0 {
                current_base = next_base;
            } else {
                break (next_base, current_metadata.clone());
            }
        };

        if next_base == addr + 4096_u64 {
            let next_meta = unsafe { get_at_addr::<RegionMetadata, _>(next_base) };

            unsafe {
                set_at_addr(
                    addr,
                    RegionMetadata {
                        size_pages: next_meta.size_pages + 1,
                        next: next_meta.next,
                    },
                )
            }
        } else {
            unsafe {
                set_at_addr(
                    addr,
                    RegionMetadata {
                        next: next_base,
                        size_pages: 1,
                    },
                )
            }
        }

        if current_base + (current_metadata.size_pages * 4096) == addr {
            let next_meta = unsafe { get_at_addr::<RegionMetadata, _>(addr) };

            unsafe {
                set_at_addr(
                    current_base,
                    RegionMetadata {
                        next: next_meta.next,
                        size_pages: current_metadata.size_pages + next_meta.size_pages,
                    },
                )
            }
        } else {
            unsafe {
                set_at_addr(
                    current_base,
                    RegionMetadata {
                        next: addr,
                        size_pages: current_metadata.size_pages,
                    },
                )
            }
        }
    }
}

impl PhysicalAllocator for SimplePhysicalAllocator {
    fn allocate(&mut self) -> OwnedPhysAddr {
        let range = self.allocate_contiguous(1);
        let addr = OwnedPhysAddr(range.0.start);
        core::mem::forget(range);
        addr
    }

    fn allocate_contiguous(&mut self, n_pages: u32) -> OwnedPhysRange {
        let mut current_base = self.start_region;
        let mut prev_base = None;
        loop {
            let current_metadata = unsafe { get_at_addr::<RegionMetadata, _>(current_base) };
            if current_metadata.size_pages >= n_pages as u64 {
                break;
            }
            prev_base = Some(current_base);
            current_base = current_metadata.next;
            if current_base == PhysAddr(0) {
                panic!("OOM");
            }
        }

        let current_metadata = unsafe { get_at_addr::<RegionMetadata, _>(current_base) }.clone();
        let new_meta = RegionMetadata {
            size_pages: current_metadata.size_pages - n_pages as u64,
            next: current_metadata.next,
        };

        let next_base = if new_meta.size_pages == 0 {
            new_meta.next
        } else {
            let next_base = current_base + 4096 * n_pages as u64;
            unsafe {
                set_at_addr(next_base, new_meta);
            };
            next_base
        };

        if let Some(prev_base) = prev_base {
            unsafe {
                set_at_addr(prev_base + core::mem::offset_of!(RegionMetadata, next) as u64, next_base);
            };
        } else {
            self.start_region = next_base;
        }

        OwnedPhysRange(PhysRange {
            start: current_base,
            n_pages: n_pages as u64,
        })
    }

    fn deallocate<T: Into<OwnedPhysRange>>(&mut self, addr: T) {
        let range: OwnedPhysRange = addr.into();
        let start_page = range.0.start.0 / 4096;
        for page in start_page..(start_page + range.0.n_pages as u64) {
            self.deallocate_single(PhysAddr(page * 4096));
        }
    }

    fn reserve_low(&mut self) -> OwnedPhysAddr {
        self.allocate() //no special handling
    }
}

pub fn init(mem_regions: &mut [&'static mut limine::MemoryMapEntry]) {
    let mut previous_region = None;
    let mut first_region = PhysAddr(0);
    for region in mem_regions {
        if !region.is_usable() {
            continue;
        }

        let metadata = RegionMetadata {
            size_pages: region.length.div_ceil(4096),
            next: PhysAddr(0),
        };
        unsafe { set_at_addr(PhysAddr(region.base), metadata) };
        println!(
            "Created a region at base {:X?} with size frames {:X}",
            region.base,
            region.length.div_ceil(4096)
        );

        if let Some(prev_reg) = previous_region {
            unsafe {
                set_at_addr(
                    prev_reg + core::mem::offset_of!(RegionMetadata, next) as u64,
                    PhysAddr(region.base),
                );
            }
        } else {
            first_region = PhysAddr(region.base)
        }

        previous_region = Some(PhysAddr(region.base))
    }

    lock_w_info!(SIMPLE_PHYS_ALLOCATOR).start_region = first_region;
    println!("set first region to {:X?}", first_region);
}

pub fn allocate_frame() -> OwnedPhysAddr {
    lock_w_info!(SIMPLE_PHYS_ALLOCATOR).allocate()
}

pub fn allocate_contiguous(n_pages: u32) -> OwnedPhysRange {
    lock_w_info!(SIMPLE_PHYS_ALLOCATOR).allocate_contiguous(n_pages)
}

pub unsafe fn deallocate<T: Into<OwnedPhysRange>>(addr: T) {
    lock_w_info!(SIMPLE_PHYS_ALLOCATOR).deallocate(addr);
}

pub fn reserve_low() -> OwnedPhysAddr {
    lock_w_info!(SIMPLE_PHYS_ALLOCATOR).reserve_low()
}
