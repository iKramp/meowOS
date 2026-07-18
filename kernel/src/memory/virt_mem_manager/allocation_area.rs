use bitfield::bitfield;
use core::ops::RangeTo;
use std::{collections::btree_set::BTreeSet, lock_w_info, println, sync::no_int_spinlock::NoIntSpinlock};

use crate::memory::addresses::*;

//match impl with the one below
bitfield! {
    #[derive(Debug, PartialEq, Eq)]
    pub struct AllocationAreaFlags(u8);
    pub writable, set_writable: 0;
    pub user_accessible, set_user_accessible: 1;
    pub page_write_through, set_page_write_through: 2;
    pub page_cache_disable, set_page_cache_disable: 3;
    pub no_execute, set_no_execute: 4;
    pub no_merge_info, set_no_merge_info: 5; //for when we're lazy and don't want to save number of
                                             //allocated pages
}

impl AllocationAreaFlags {
    pub fn default() -> Self {
        let mut tmp = Self(0);
        tmp.set_writable(true);
        tmp.set_user_accessible(false);
        tmp.set_page_write_through(false);
        tmp.set_page_cache_disable(false);
        tmp.set_no_execute(false);
        tmp.set_no_merge_info(false);
        tmp
    }
}

struct AllocationInfo {
    free_size_sorted: BTreeSet<SizeSortedAllocArea>,
    free_addr_sorted: BTreeSet<AddrSortedAllocArea>,
    used: BTreeSet<AddrSortedAllocArea>,
}
static ALLOCATION_INFO: NoIntSpinlock<AllocationInfo> = NoIntSpinlock::new(AllocationInfo {
    free_size_sorted: BTreeSet::new(),
    free_addr_sorted: BTreeSet::new(),
    used: BTreeSet::new(),
});
static mut INITIALIZED: bool = false;

pub(super) fn init(empty_areas: &[(VirtAddr, u64)]) {
    let mut info = lock_w_info!(ALLOCATION_INFO);
    info.free_addr_sorted.clear();
    info.free_size_sorted.clear();
    for (addr, n_pages) in empty_areas {
        let mut addr = *addr;
        let mut n_pages = *n_pages;
        const MIN_ADDR: u64 = 0x800000000000;
        const MAX_ADDR: u64 = 0xFFFFFFFFE000;
        if addr.0 < MIN_ADDR {
            continue;
        }
        let min_addr_diff = MIN_ADDR.saturating_sub(addr.0);
        addr.0 += min_addr_diff;
        n_pages = n_pages.saturating_sub(min_addr_diff >> 12);
        if n_pages == 0 {
            continue;
        }

        if addr.0.checked_add(n_pages * 0x1000).is_none() {
            n_pages -= 1;
        }

        println!("adding free area: addr: {:x}, size: {:x} pages", addr.0, n_pages);
        let mut area = AllocArea(0);
        area.set_start_page_index(addr.0 >> 12);
        let size_sorted = SizeSortedAllocArea { inner: area, n_pages };
        let addr_sorted = AddrSortedAllocArea { inner: area, n_pages };
        info.free_size_sorted.insert(size_sorted);
        info.free_addr_sorted.insert(addr_sorted);
    }

    unsafe {
        INITIALIZED = true;
    }
}

bitfield! {
    struct AllocArea(u64);
    impl Debug;
    start_page_index, set_start_page_index: 47, 0;
    pub writable, set_writable: 48;
    pub user_accessible, set_user_accessible: 49;
    pub page_write_through, set_page_write_through: 50;
    pub page_cache_disable, set_page_cache_disable: 51;
    pub no_execute, set_no_execute: 52;
    //for when we're lazy and don't want to save number of allocated pages
    pub no_merge_info, set_no_merge_info: 53;
}

impl AllocArea {
    pub fn flags(&self) -> u32 {
        ((self.0 << 48) & 0b111111) as u32
    }
}

#[derive(Clone, Copy, Debug)]
struct SizeSortedAllocArea {
    inner: AllocArea,
    n_pages: u64,
}

#[derive(Clone, Copy, Debug)]
struct AddrSortedAllocArea {
    inner: AllocArea,
    n_pages: u64,
}

pub fn allocate_area(n_pages: u64, flags: AllocationAreaFlags) -> Option<VirtAddr> {
    println!("allocating virt mem area: n_pages {}, flags: {:?}", n_pages, flags);
    //print current state
    let mut info = lock_w_info!(ALLOCATION_INFO);

    println!("state before:");
    println!("free size sorted:");
    for element in info.free_size_sorted.iter() {
        println!("{:X?}", element);
    }
    println!("free addr sorted:");
    for element in info.free_addr_sorted.iter() {
        println!("{:X?}", element);
    }
    println!("allocated:");
    for element in info.used.iter() {
        println!("{:X?}", element);
    }

    let mut iterator = info.free_size_sorted.range(
        SizeSortedAllocArea {
            inner: AllocArea(0),
            n_pages,
        }..,
    );
    let entry = iterator.next()?;
    let entry = *entry;
    drop(iterator);

    info.free_size_sorted.remove(&entry);
    info.free_addr_sorted.remove(&entry.into());

    let mut new_empty = entry;

    new_empty
        .inner
        .set_start_page_index(new_empty.inner.start_page_index() + n_pages);
    new_empty.n_pages -= n_pages;

    if new_empty.n_pages > 0 {
        info.free_addr_sorted.insert(new_empty.into());
        info.free_size_sorted.insert(new_empty);
    }

    let mut new_alloc = AddrSortedAllocArea {
        inner: AllocArea((flags.0 as u64 & 0b111111) << 48),
        n_pages: 0,
    };
    new_alloc.inner.set_start_page_index(entry.inner.start_page_index());
    new_alloc.n_pages = n_pages;

    if !flags.no_merge_info() {
        let testing_struct = AddrSortedAllocArea {
            inner: AllocArea(entry.inner.start_page_index() + n_pages),
            n_pages: 0,
        };
        if let Some(entry) = info.used.get(&testing_struct) {
            if new_alloc.inner.flags() == entry.inner.flags() {
                //also checks that entry can be merged
                new_alloc.n_pages += entry.n_pages;
                let entry = *entry;
                info.used.remove(&entry);
            }
        }

        if let Some(entry) = info.used.range(..new_alloc).next_back() {
            if entry.inner.start_page_index() + entry.n_pages == new_alloc.inner.start_page_index()
                && new_alloc.inner.flags() == entry.inner.flags()
            {
                new_alloc.inner.set_start_page_index(entry.inner.start_page_index());
                new_alloc.n_pages += entry.n_pages;
                let entry = *entry;
                info.used.remove(&entry);
            }
        }
    }

    info.used.insert(new_alloc);

    let addr = VirtAddr(entry.inner.start_page_index() * 0x1000);

    println!("state after:");
    println!("free size sorted:");
    for element in info.free_size_sorted.iter() {
        println!("{:X?}", element);
    }
    println!("free addr sorted:");
    for element in info.free_addr_sorted.iter() {
        println!("{:X?}", element);
    }
    println!("allocated:");
    for element in info.used.iter() {
        println!("{:X?}", element);
    }

    Some(extend_addr(addr))
}

pub fn free_area(addr: VirtAddr, n_pages: Option<u64>) {
    let addr = trim_addr_extension(addr);
    let addr_page = addr.0 >> 12;
    let n_pages = n_pages.unwrap_or(u64::MAX);
    let mut info = lock_w_info!(ALLOCATION_INFO);
    let Some(entry) = info
        .used
        .range(
            ..=AddrSortedAllocArea {
                inner: AllocArea(addr_page),
                n_pages: 0,
            },
        )
        .next_back()
    else {
        debug_assert!(false, "tried to free unallocated area {:x}", addr.0);
        return;
    };
    if entry.inner.start_page_index() + entry.n_pages <= addr_page {
        debug_assert!(false, "tried to free unallocated area {:x}", addr.0);
        return;
    }

    let entry = *entry;
    info.used.remove(&entry);
    let n_pages = n_pages.min(entry.inner.start_page_index() + entry.n_pages - addr_page);
    let left_in_front = (addr_page - entry.inner.start_page_index()) / 0x1000;
    let left_after = entry.n_pages - left_in_front - n_pages;

    if left_in_front > 0 {
        let mut tmp = AllocArea(entry.inner.0); //copy flags
        tmp.set_start_page_index(entry.inner.start_page_index());

        info.used.insert(AddrSortedAllocArea {
            inner: tmp,
            n_pages: left_in_front,
        });
    }
    if left_after > 0 {
        let mut tmp = AllocArea(entry.inner.0); //copy flags
        tmp.set_start_page_index(addr_page + n_pages);

        info.used.insert(AddrSortedAllocArea {
            inner: tmp,
            n_pages: left_after,
        });
    }

    let mut new_free = {
        let mut tmp = AllocArea(0);
        tmp.set_start_page_index(addr_page);
        SizeSortedAllocArea { inner: tmp, n_pages }
    };

    let testing_struct = AddrSortedAllocArea {
        inner: AllocArea(addr_page + n_pages),
        n_pages: 0,
    };
    if let Some(entry) = info.free_addr_sorted.get(&testing_struct) {
        new_free.n_pages += entry.n_pages;
        let entry = *entry;
        info.free_addr_sorted.remove(&entry);
        info.free_size_sorted.remove(&entry.into());
    }
    if let Some(entry) = info
        .free_addr_sorted
        .range::<AddrSortedAllocArea, RangeTo<AddrSortedAllocArea>>(..new_free.into())
        .next_back()
    {
        if entry.inner.start_page_index() + entry.n_pages == new_free.inner.start_page_index() {
            new_free.inner.set_start_page_index(entry.inner.start_page_index());
            new_free.n_pages += entry.n_pages;
            let entry = *entry;
            info.free_addr_sorted.remove(&entry);
            info.free_size_sorted.remove(&entry.into());
        }
    }

    info.free_addr_sorted.insert(new_free.into());
    info.free_size_sorted.insert(new_free);
}

impl core::cmp::Ord for AddrSortedAllocArea {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.inner.start_page_index().cmp(&other.inner.start_page_index())
    }
}
impl core::cmp::Ord for SizeSortedAllocArea {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.n_pages
            .cmp(&other.n_pages)
            .then_with(|| self.inner.start_page_index().cmp(&other.inner.start_page_index()))
    }
}

impl core::cmp::PartialOrd for AddrSortedAllocArea {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl core::cmp::PartialOrd for SizeSortedAllocArea {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::cmp::Eq for AddrSortedAllocArea {}
impl core::cmp::Eq for SizeSortedAllocArea {}
impl core::cmp::PartialEq for AddrSortedAllocArea {
    fn eq(&self, other: &Self) -> bool {
        self.inner.start_page_index() == other.inner.start_page_index()
    }
}
impl core::cmp::PartialEq for SizeSortedAllocArea {
    fn eq(&self, other: &Self) -> bool {
        self.n_pages == other.n_pages && self.inner.start_page_index() == other.inner.start_page_index()
    }
}

impl core::convert::From<AddrSortedAllocArea> for SizeSortedAllocArea {
    fn from(value: AddrSortedAllocArea) -> Self {
        Self {
            inner: value.inner,
            n_pages: value.n_pages,
        }
    }
}

impl core::convert::From<SizeSortedAllocArea> for AddrSortedAllocArea {
    fn from(value: SizeSortedAllocArea) -> Self {
        Self {
            inner: value.inner,
            n_pages: value.n_pages,
        }
    }
}
