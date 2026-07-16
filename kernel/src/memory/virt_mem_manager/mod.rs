use core::ops::Range;
use std::{
    error::ErrorCode,
    lock_w_info,
    mem_utils::{PhysAddr, VirtAddr, get_at_physical_addr, translate_phys_virt_addr, translate_virt_phys_addr},
    println,
    sync::no_int_spinlock::NoIntSpinlock,
};

use crate::memory::{
    physical_allocator,
    virt_mem_manager::{allocation_area::AllocationAreaFlags, page_table::PageTable},
};

mod page_table;
mod page_table_entry;
pub use page_table_entry::LiminePat;
pub use page_table_entry::PageTableEntry;
mod allocation_area;
mod debug_printing;
mod virtual_memory_range;
pub(super) use debug_printing::print_mem_mapping;
pub use virtual_memory_range::*;

pub(super) fn init_paging() {
    prepare_higher_half();

    let root = current_root();
    let page_table = unsafe { get_at_physical_addr::<PageTable>(root) };

    let ranges = page_table.get_free_ranges(VirtAddr(0), 4);
    println!("current paging empty areas:");
    for (addr, n_pages) in &ranges {
        println!("virt addr: {:#x?}, size: {:#x?} pages", addr, n_pages);
    }

    allocation_area::init(&ranges);
}

pub fn current_root() -> PhysAddr {
    let mut level_4_table = PhysAddr(0);
    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) level_4_table.0,
        );
    }
    level_4_table
}

pub fn set_cr3(addr: PhysAddr) {
    unsafe {
        core::arch::asm!(
            "mov cr3, {}",
            in(reg) addr.0
        );
    }
}

//TODO: remove pub when removing paging.rs
pub fn flush_tlb(addr: Option<VirtAddr>) {
    match addr {
        Some(addr) => unsafe {
            core::arch::asm!(
                "invlpg [{}]",
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
#[inline]
pub fn kernel_map(phys_addr: Option<PhysAddr>) -> VirtAddr {
    let phys_addr = match phys_addr {
        Some(a) => a,
        None => physical_allocator::allocate_frame(),
    };
    translate_phys_virt_addr(phys_addr)
}

#[inline]
pub fn kernel_free(addr: VirtAddr) {
    let phys_addr = translate_virt_phys_addr(addr, None).expect("freeing on HHDM");
    unsafe { physical_allocator::deallocate_frame(phys_addr) };
}

#[inline]
pub fn kernel_map_contiguous(phys_addr: Option<PhysAddr>, n_pages: u64) -> VirtAddr {
    let phys_addr = match phys_addr {
        Some(a) => a,
        None => physical_allocator::allocate_contiguous(n_pages as u32),
    };
    translate_phys_virt_addr(phys_addr)
}

#[inline]
pub fn kernel_free_contiguous(addr: VirtAddr, n_pages: u64) {
    let phys_addr = translate_virt_phys_addr(addr, None).expect("freeing on HHDM");
    for i in 0..n_pages {
        let addr = phys_addr.0 + i * 0x1000;
        unsafe { physical_allocator::deallocate_frame(PhysAddr(addr)) };
    }
}

pub fn kernel_unmap(_addr: VirtAddr) {
    panic!("check your logic, no unmapping HHDM");
}

static MANUAL_MAP_LOCK: NoIntSpinlock<()> = NoIntSpinlock::new(());

/// Intended to be used for MMIO, or physical ram in very rare cases.
/// Caller must ensure the phys_addr is valid and owned
/// If calling in a loop, provide page tree root
pub unsafe fn kernel_manual_map(
    phys_addr: PhysAddr,
    pages: u64,
    page_tree_root: Option<PhysAddr>,
) -> (VirtAddr, &'static mut PageTableEntry) {
    let virt_addr = allocation_area::allocate_area(pages, AllocationAreaFlags::default()).expect("OOM");

    let _lock = lock_w_info!(MANUAL_MAP_LOCK);
    let page_table_root = page_tree_root.unwrap_or_else(current_root);
    let page_table = unsafe { get_at_physical_addr::<PageTable>(page_table_root) };
    let res = unsafe { page_table.kernel_manual_map(phys_addr, virt_addr, pages, VirtAddr(0), 4) };
    drop(_lock);
    (virt_addr, res.0)
}

/// Intended to be used for MMIO, or physical ram in very rare cases.
/// Caller must release the physical memory
/// If calling in a loop, provide page tree root
pub unsafe fn kernel_manual_unmap(virt_addr: VirtAddr, pages: u64, page_tree_root: Option<PhysAddr>) {
    allocation_area::free_area(virt_addr, Some(pages));

    let _lock = lock_w_info!(MANUAL_MAP_LOCK);
    let page_table_root = page_tree_root.unwrap_or_else(current_root);
    let page_table = unsafe { get_at_physical_addr::<PageTable>(page_table_root) };
    unsafe { page_table.kernel_manual_unmap(virt_addr, pages, VirtAddr(0), 4) };
    drop(_lock);
}

pub fn userspace_map(
    page_range: Range<u32>,
    permissions: VirtualMemoryRangePermissions,
    table_phys: PhysAddr,
    table_level: u8,
    table_page_index: u32,
) -> Result<(), ErrorCode> {
    assert!((1..=3).contains(&table_level));

    let table = unsafe { get_at_physical_addr::<PageTable>(table_phys) };
    table.userspace_map(page_range, permissions, table_level, table_page_index);
    Ok(())
}

pub fn userspace_unmap(pages: Range<u32>, table_phys: PhysAddr, table_level: u8, table_page_index: u32) -> Result<(), ErrorCode> {
    assert!((1..=3).contains(&table_level));

    let table = unsafe { get_at_physical_addr::<PageTable>(table_phys) };
    table.userspace_unmap(pages, table_level, table_page_index);
    Ok(())
}

pub fn set_prot(
    table_phys: PhysAddr,
    addr_range: Range<VirtAddr>,
    permissions: VirtualMemoryRangePermissions,
    table_level: u8,
    table_addr: VirtAddr,
) {
    assert!((1..=4).contains(&table_level));
    let page_table = unsafe { get_at_physical_addr::<PageTable>(table_phys) };
    page_table.set_prot(addr_range, permissions, table_level, table_addr);
}

pub fn get_page_table_entry(virt_addr: VirtAddr, page_tree_root: Option<PhysAddr>) -> Option<&'static mut PageTableEntry> {
    let page_tree_root = page_tree_root.unwrap_or_else(current_root);
    get_page_table_entry_at_level(page_tree_root, virt_addr, 1, false)
}

pub fn get_page_table_entry_at_level(
    root: PhysAddr,
    virt_addr: VirtAddr,
    level: u8,
    allocate_missing: bool,
) -> Option<&'static mut PageTableEntry> {
    assert!((1..=4).contains(&level));
    let page_table = unsafe { get_at_physical_addr::<PageTable>(root) };
    page_table.get_page_table_entry(virt_addr, VirtAddr(0), 4, level, allocate_missing)
}

pub fn unmap_lower_half() {
    let page_tree_root = current_root();
    let page_table = unsafe { get_at_physical_addr::<PageTable>(page_tree_root) };
    for entry in &mut page_table.entries[..256] {
        if !entry.present() {
            continue;
        }
        if entry.huge_page() {
            panic!("not dealing with huge pages at level 4");
        }
        //don't delete lower entries, limine shares them with HHDM
        entry.set_present(false);
    }
}

pub fn delete_page_table(root: PhysAddr, level: u8, unmap_phys: bool) {
    PageTable::delete(root, level, unmap_phys);
}

pub fn prepare_higher_half() {
    let page_tree_root = current_root();
    let page_table = unsafe { get_at_physical_addr::<PageTable>(page_tree_root) };
    for entry in &mut page_table.entries[256..] {
        if entry.present() {
            continue;
        }
        let frame = physical_allocator::allocate_frame();
        unsafe {
            core::ptr::write_volatile(
                get_at_physical_addr::<PageTable>(frame),
                PageTable {
                    entries: [PageTableEntry(0); 512],
                },
            )
        };
        *entry = PageTableEntry::new(frame, false);
    }
}

pub fn copy_higher_half(existing_tree: PhysAddr, new_tree: PhysAddr) {
    unsafe {
        let existing_page_table = get_at_physical_addr::<PageTable>(existing_tree);
        let new_page_table = get_at_physical_addr::<PageTable>(new_tree);
        for i in 256..512 {
            new_page_table.entries[i] = existing_page_table.entries[i];
        }
    }
}
