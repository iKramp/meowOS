use core::ops::Range;
use std::mem_utils::{PhysAddr, VirtAddr, translate_phys_virt_addr, translate_virt_phys_addr};

use crate::memory::{self, physical_allocator};

mod page_table;
mod page_table_entry;
mod physical_memory_range;
mod virtual_memory_range;

pub(super) fn init_paging() {}

fn current_root() -> PhysAddr {
    let mut level_4_table = PhysAddr(0);
    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) level_4_table.0,
        );
    }
    level_4_table
}

fn set_cr3(addr: PhysAddr) {
    unsafe {
        core::arch::asm!(
            "mov cr3, {}",
            in(reg) addr.0
        );
    }
}

fn flush_tlb(addr: Option<VirtAddr>) {
    match addr {
        Some(addr) => unsafe {
            core::arch::asm!(
                "invlpg {}",
                in(reg) addr.0
            )
        },
        None => {
            set_cr3(current_root());
        }
    }
}

enum PageMapAddrRequest {
    Any,
    Exact(VirtAddr),
    Range(Range<VirtAddr>),
}

//publlic API
pub fn kernel_map(phys_addr: Option<PhysAddr>) -> VirtAddr {
    let phys_addr = match phys_addr {
        Some(a) => a,
        None => physical_allocator::allocate_frame(),
    };
    translate_phys_virt_addr(phys_addr)
}

pub fn kernel_free(addr: VirtAddr) {
    let phys_addr = translate_virt_phys_addr(addr, unsafe { memory::PAGE_TREE_ALLOCATOR.root() });
    if let Some(phys_addr) = phys_addr {
        unsafe { physical_allocator::deallocate_frame(phys_addr) };
    }
}

pub fn kernel_map_contiguous(phys_addr: Option<PhysAddr>, n_pages: u64) -> VirtAddr {
    let phys_addr = match phys_addr {
        Some(a) => a,
        None => physical_allocator::allocate_contiguius_high(n_pages),
    };
    translate_phys_virt_addr(phys_addr)
}

pub fn kernel_unmap(_addr: VirtAddr) {
    panic!("check your logic, no unmapping HHDM");
}
