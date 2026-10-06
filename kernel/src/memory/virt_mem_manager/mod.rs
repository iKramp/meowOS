use core::ops::Range;
use std::{error::KernelError, lock_w_info, println, sync::no_int_spinlock::NoIntSpinlock};

use crate::{
    arch::x86_64::memory::{
        PAGE_TABLE_ENTRIES, PAGING_LEVELS, current_root, page_table::PageTable, page_table_entry::PageTableEntry,
    },
    memory::{addresses::*, physical_allocator, virt_mem_manager::allocation_area::AllocationAreaFlags},
};

mod allocation_area;
mod debug_printing;
mod virtual_memory_range;
pub(super) use debug_printing::print_mem_mapping;
pub use virtual_memory_range::*;

/// Memory type | Cacheable | Writeback cacheable | Speculative reads             
/// ----------- | --------- | ------------------- | -----------------
/// UC | No | No | No
/// WC | No | No | Yes
/// WT | Yes | No | Yes
/// WB | Yes | Yes | Yes
/// WP | Yes(R)/No(W) | No | Yes
///
/// Documenation from the Intel SDM:
///
/// Uncacheable (UC) — System memory locations are not cached. All reads and writes appear on the
/// system bus and are executed in program order without reordering. No speculative memory accesses, page-
/// table walks, or prefetches of speculated branch targets are made. This type of cache-control is useful for
/// memory-mapped I/O devices. When used with normal RAM, it greatly reduces processor performance.
///
/// Write Combining (WC) — System memory locations are not cached. Writes to WC memory locations are
/// combined in a write buffer and written to memory in program order. Reads are not combined.
/// Speculative memory accesses, page-table walks, and prefetches of speculated branch targets are
/// made.
///
/// Write Through (WT) — Writes and reads to and from system memory are cached. Reads come from cache
/// lines on cache hits; read misses cause cache fills. Speculative reads are allowed. All writes are written to a
/// cache line (when possible) and through to system memory. When writing through to memory, invalid cache
/// lines are never filled, and valid cache lines are either filled or invalidated. Write combining is allowed. This type
/// of cache-control is appropriate for frame buffers or when there are devices on the system bus that access
/// system memory, but do not perform snooping of memory accesses. It enforces coherency between caches in
/// the processors and system memory.
///
/// Write Back (WB) — Writes and reads to and from system memory are cached. Reads come from cache lines
/// on cache hits; read misses cause cache fills. Speculative reads are allowed. Write misses cause cache line fills,
/// and writes are performed entirely in the cache, when possible. Write combining is allowed.
/// The write-back memory type reduces bus traffic by eliminating many unnecessary writes to system memory.
/// Writes to a cache line are not immediately forwarded to system memory; instead,
/// they are accumulated in the cache. The modified cache lines are written to system memory
/// later, when a write-back operation is performed. Write-back operations are triggered when cache lines need to
/// be deallocated, such as when new cache lines are being allocated in a cache that is already full. They also are
/// triggered by the mechanisms used to maintain cache consistency. This type of cache-control provides the best
/// performance, but it requires that all devices that access system memory on the system bus be able to snoop
/// memory accesses to ensure system memory and cache coherency.
///
/// Write protected (WP) — Reads come from cache lines when possible, and read misses cause cache fills.
/// Writes are propagated to the system bus and cause corresponding cache lines on all processors on the bus to
/// be invalidated. Speculative reads are allowed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCachingStrategy {
    Uncacheable,
    WriteCombining,
    WriteThrough,
    WriteBack,
    WriteProtected,
}

pub(super) fn init_paging() {
    prepare_higher_half();

    let root = current_root();
    let page_table = unsafe { get_at_addr::<PageTable, _>(root) };

    let ranges = page_table.get_free_ranges(VirtAddr(0), PAGING_LEVELS);
    println!("current paging empty areas:");
    for (addr, n_pages) in &ranges {
        println!("virt addr: {:#x?}, size: {:#x?} pages", addr, n_pages);
    }

    allocation_area::init(&ranges);
}

enum PageMapAddrRequest {
    Any,
    Exact(VirtAddr),
    Range(Range<VirtAddr>),
}

//publlic API
#[inline]
pub fn kernel_map(phys_addr: OwnedPhysAddr) -> OwnedVirtAddr {
    phys_addr.into()
}

#[inline]
pub fn kernel_free(addr: OwnedVirtAddr) {
    assert!(is_in_hhdm(addr.0), "kernel-freeing non-HHDM address");
    let phys_addr = translate_virt_phys_addr(addr.0, None).expect("freeing on HHDM");
    let owned_phys_addr = OwnedPhysAddr(phys_addr);
    core::mem::forget(addr);
    drop(owned_phys_addr);
}

#[inline]
pub fn kernel_map_contiguous(phys_range: OwnedPhysRange) -> OwnedVirtRange {
    phys_range.into()
}

#[inline]
pub fn kernel_free_contiguous(range: OwnedVirtRange) {
    assert!(is_in_hhdm(range.0.start), "kernel-freeing non-HHDM address");
    let phys_range = translate_virt_phys_addr(range.0.start, None).expect("freeing on HHDM");
    let owned_phys_range = OwnedPhysRange(PhysRange {
        start: phys_range,
        n_pages: range.0.n_pages,
    });
    core::mem::forget(range);
    drop(owned_phys_range);
}

pub fn kernel_unmap(_addr: VirtAddr) {
    panic!("check your logic, no unmapping HHDM");
}

static MANUAL_MAP_LOCK: NoIntSpinlock<()> = NoIntSpinlock::new(());

/// Intended to be used for MMIO, or physical ram in very rare cases.
/// Caller must ensure the phys_addr is valid and owned
/// If calling in a loop, provide page tree root
pub unsafe fn kernel_manual_map(
    phys_addr: OwnedPhysRange,
    page_tree_root: Option<PhysAddr>,
) -> (OwnedVirtRange, &'static mut PageTableEntry) {
    let pages = phys_addr.0.n_pages;
    let virt_addr = allocation_area::allocate_area(pages, AllocationAreaFlags::default()).expect("OOM");

    let _lock = lock_w_info!(MANUAL_MAP_LOCK);
    let page_table_root = page_tree_root.unwrap_or_else(current_root);
    let page_table = unsafe { get_at_addr::<PageTable, _>(page_table_root) };
    let res = unsafe { page_table.kernel_manual_map(phys_addr.0.start, virt_addr, pages, VirtAddr(0), PAGING_LEVELS) };
    drop(_lock);

    let virt_range = VirtRange {
        start: virt_addr,
        n_pages: pages,
    };
    let owned_virt_range = OwnedVirtRange(virt_range);
    core::mem::forget(phys_addr);

    (owned_virt_range, res.0)
}

/// Intended to be used for MMIO, or physical ram in very rare cases.
/// Caller must release the physical memory
/// If calling in a loop, provide page tree root
pub unsafe fn kernel_manual_unmap(virt_addr: VirtAddr, pages: u64, page_tree_root: Option<PhysAddr>) {
    allocation_area::free_area(virt_addr, Some(pages));

    let _lock = lock_w_info!(MANUAL_MAP_LOCK);
    let page_table_root = page_tree_root.unwrap_or_else(current_root);
    let page_table = unsafe { get_at_addr::<PageTable, _>(page_table_root) };
    unsafe { page_table.kernel_manual_unmap(virt_addr, pages, VirtAddr(0), PAGING_LEVELS) };
    drop(_lock);
}

pub fn userspace_map(
    page_range: Range<u32>,
    permissions: VirtualMemoryRangePermissions,
    table_phys: PhysAddr,
    table_level: u8,
    table_page_index: u32,
) -> Result<(), KernelError> {
    assert!((1..PAGING_LEVELS).contains(&table_level));

    let table = unsafe { get_at_addr::<PageTable, _>(table_phys) };
    table.userspace_map(page_range, permissions, table_level, table_page_index);
    Ok(())
}

pub fn userspace_unmap(
    pages: Range<u32>,
    table_phys: PhysAddr,
    table_level: u8,
    table_page_index: u32,
) -> Result<(), KernelError> {
    assert!((1..PAGING_LEVELS).contains(&table_level));

    let table = unsafe { get_at_addr::<PageTable, _>(table_phys) };
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
    assert!((1..=PAGING_LEVELS).contains(&table_level));
    let page_table = unsafe { get_at_addr::<PageTable, _>(table_phys) };
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
    let page_table = unsafe { get_at_addr::<PageTable, _>(root) };
    page_table.get_page_table_entry(virt_addr, VirtAddr(0), 4, level, allocate_missing)
}

pub fn unmap_lower_half() {
    let page_tree_root = current_root();
    let page_table = unsafe { get_at_addr::<PageTable, _>(page_tree_root) };
    for entry in page_table.entries_mut().take(PAGE_TABLE_ENTRIES as usize / 2) {
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
    let page_table = unsafe { get_at_addr::<PageTable, _>(page_tree_root) };
    for entry in page_table.entries_mut().skip(PAGE_TABLE_ENTRIES as usize / 2) {
        if entry.present() {
            continue;
        }
        let frame = physical_allocator::allocate();
        unsafe { core::ptr::write_volatile(get_at_addr::<PageTable, _>(&frame), PageTable::new_empty()) };
        *entry = PageTableEntry::new(frame.0, false);
        core::mem::forget(frame); //don't deallocate
    }
}

pub fn copy_higher_half(existing_tree: PhysAddr, new_tree: PhysAddr) {
    unsafe {
        let existing_page_table = get_at_addr::<PageTable, _>(existing_tree);
        let new_page_table = get_at_addr::<PageTable, _>(new_tree);
        for (existing, new) in existing_page_table
            .entries()
            .zip(new_page_table.entries_mut())
            .skip(PAGE_TABLE_ENTRIES as usize / 2)
        {
            *new = *existing;
        }
    }
}
