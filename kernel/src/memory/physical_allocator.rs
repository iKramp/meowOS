use std::lock_w_info;
use std::sync::no_int_spinlock::NoIntSpinlock;

use crate::memory::{heap::log2_rounded_up, printlnc};

use crate::{limine, println};

use super::mem_utils::*;

static BUDDY_ALLOCATOR: NoIntSpinlock<BuddyAllocator> = NoIntSpinlock::new(BuddyAllocator {
    n_pages: 0,
    binary_tree_size: 0,
    allocated_pages: 0,
    tree_allocator: VirtAddr(0),
});

pub static mut MAX_RAM_ADDR: PhysAddr = PhysAddr(0);

pub fn is_on_ram(addr: PhysAddr) -> bool {
    addr.0 <= unsafe { MAX_RAM_ADDR.0 }
}

pub struct BuddyAllocator {
    ///number of pages that can be allocated in physical address space. Is not a power of 2
    pub n_pages: u64,
    ///number of nodes this binary tree has, plus the zero-th node (unused). IS always a power of 2
    binary_tree_size: u64,
    allocated_pages: u64,
    tree_allocator: VirtAddr,
}

pub fn init() {
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

    //is a power of 2
    let binary_tree_size_elements = get_binary_tree_element_cnt(n_pages);
    // div by 8 for 8 bits in a byte  (also rounded up), times 2 for binary tree
    let space_needed_bytes = binary_tree_size_elements / 8 * 2;
    let entry_to_shrink = find_mem_region_to_shrink(memory_regions, space_needed_bytes);

    let tree_allocator = PhysAddr(memory_regions[entry_to_shrink].base);
    let mut allocated_pages = 0;
    for i in 0..space_needed_bytes {
        //set all bits to 1 to set everything as used
        unsafe {
            set_at_physical_addr(tree_allocator + PhysAddr(i), 0xFF_u8);
            allocated_pages += 4;
        }
    }

    //at least one page, space_needed is a multiple of that if bigger
    let size_to_shrink = u64::max(0x1000, space_needed_bytes);
    memory_regions[entry_to_shrink].base += size_to_shrink;
    memory_regions[entry_to_shrink].length -= size_to_shrink;
    let mut allocator = BuddyAllocator {
        n_pages,
        binary_tree_size: binary_tree_size_elements * 2,
        allocated_pages,
        tree_allocator: translate_phys_virt_addr(tree_allocator),
    };
    for entry in memory_regions {
        if !is_memory_region_usable(entry) {
            continue;
        }
        for addr in (entry.base..(entry.base + entry.length)).step_by(0x1000) {
            let index = (addr >> 12) + (allocator.binary_tree_size / 2);
            allocator.set_at_index(index, false);
            allocator.allocated_pages -= 1;
        }
    }
    allocator.update_all();
    *lock_w_info!(BUDDY_ALLOCATOR) = allocator;
}

pub fn is_frame_allocated(addr: PhysAddr) -> bool {
    lock_w_info!(BUDDY_ALLOCATOR).is_frame_allocated(addr)
}

pub fn print_state() {
    let allocator = lock_w_info!(BUDDY_ALLOCATOR);
    printlnc!((255, 200, 100), "Buddy Allocator state:");
    println!("all_frames: {}", allocator.n_pages);
    println!("allocated_frames: {}", allocator.allocated_pages);
}

///# Safety
///addr must be a page aligned, currently allocated physical frame address
pub unsafe fn deallocate_frame(addr: PhysAddr) {
    lock_w_info!(BUDDY_ALLOCATOR).deallocate_frame(addr)
}

pub fn allocate_frame() -> PhysAddr {
    lock_w_info!(BUDDY_ALLOCATOR).allocate_frame()
}

pub fn allocate_frame_low() -> PhysAddr {
    lock_w_info!(BUDDY_ALLOCATOR).allocate_frame_low()
}

pub fn allocate_contiguius_high(n_pages: u64) -> PhysAddr {
    lock_w_info!(BUDDY_ALLOCATOR).allocate_contiguius_high(n_pages)
}

pub fn allocate_contiguius_low(n_pages: u64) -> PhysAddr {
    lock_w_info!(BUDDY_ALLOCATOR).allocate_contiguius_low(n_pages)
}

///# Safety
///addr must be a page aligned physical frame address
pub unsafe fn mark_addr(addr: PhysAddr, allocated: bool) {
    let allocator = lock_w_info!(BUDDY_ALLOCATOR);
    if (addr.0 >> 12) >= allocator.n_pages {
        //we're dealing with mmio
        return;
    }
    allocator.mark_addr(addr, allocated)
}

impl BuddyAllocator {
    fn is_frame_allocated(&self, addr: PhysAddr) -> bool {
        #[cfg(debug_assertions)]
        assert!(
            addr.0 & 0xFFF == 0,
            "error in is_frame_allocated at addr {}: address is not page aligned",
            addr.0,
        );
        self.get_at_index((addr.0 >> 12) + (self.binary_tree_size / 2))
    }

    fn deallocate_frame(&mut self, addr: PhysAddr) {
        self.mark_addr(addr, false);
        self.allocated_pages -= 1;
    }

    fn allocate_frame(&mut self) -> PhysAddr {
        if self.allocated_pages == self.binary_tree_size / 2 {
            panic!("no more frames to allocate");
        }
        self.allocated_pages += 1;
        let index = self.find_empty_frame_high();
        self.mark_index(index, true);
        let address = (index - self.binary_tree_size / 2) * 4096;
        debug_assert!(address <= self.n_pages * 4096, "address is out of bounds");
        PhysAddr(address)
    }

    fn allocate_frame_low(&mut self) -> PhysAddr {
        if self.allocated_pages == self.binary_tree_size / 2 {
            panic!("no more frames to allocate");
        }
        self.allocated_pages += 1;
        let index = self.find_empty_frame_low();
        self.mark_index(index, true);
        let address = (index - self.binary_tree_size / 2) * 4096;
        debug_assert!(address <= self.n_pages * 4096, "address is out of bounds");
        PhysAddr(address)
    }

    fn find_empty_frame_high(&self) -> u64 {
        assert!(!self.get_at_index(1), "root node of physical memory allocator is filled");
        self.find_empty_frame_recursively_high(1)
    }

    fn find_empty_frame_recursively_high(&self, curr_index: u64) -> u64 {
        if curr_index >= self.binary_tree_size / 2 {
            //is in second half of the tree, so last level
            return curr_index;
        }
        debug_assert!(
            !self.get_at_index(curr_index),
            "asked to find empty page from this index {} but all sub-regions are filled",
            curr_index
        );
        if !self.get_at_index(curr_index * 2 + 1) {
            self.find_empty_frame_recursively_high(curr_index * 2 + 1)
        } else {
            self.find_empty_frame_recursively_high(curr_index * 2)
        }
    }

    fn find_empty_frame_low(&self) -> u64 {
        assert!(!self.get_at_index(1), "root node of physical memory allocator is filled");
        self.find_empty_frame_recursively_low(1)
    }

    fn find_empty_frame_recursively_low(&self, curr_index: u64) -> u64 {
        if curr_index >= self.binary_tree_size / 2 {
            //is in second half of the tree, so last level
            return curr_index;
        }
        debug_assert!(
            !self.get_at_index(curr_index),
            "asked to find empty page from this index {} but all sub-regions are filled",
            curr_index
        );
        if !self.get_at_index(curr_index * 2) {
            self.find_empty_frame_recursively_low(curr_index * 2)
        } else {
            self.find_empty_frame_recursively_low(curr_index * 2 + 1)
        }
    }

    fn allocate_contiguius_high(&mut self, n_pages: u64) -> PhysAddr {
        let index = self.find_contigious_empty_high(n_pages);
        for i in index..index + n_pages {
            self.mark_index(i, true);
        }
        let address = (index - self.binary_tree_size / 2) * 4096;
        debug_assert!(address <= self.n_pages * 4096, "address is out of bounds");
        PhysAddr(address)
    }

    fn allocate_contiguius_low(&mut self, n_pages: u64) -> PhysAddr {
        let index = self.find_contigious_empty_low(n_pages);
        for i in index..index + n_pages {
            self.mark_index(i, true);
        }
        let address = (index - self.binary_tree_size / 2) * 4096;
        debug_assert!(address <= self.n_pages * 4096, "address is out of bounds");
        PhysAddr(address)
    }

    fn find_contigious_empty_low(&self, n_pages: u64) -> u64 {
        let order = log2_rounded_up(n_pages);
        self.find_contigious_empty_recursively_low(1, order).expect("OOM in physical memory allocator")
    }

    fn find_contigious_empty_high(&self, n_pages: u64) -> u64 {
        let order = log2_rounded_up(n_pages);
        self.find_contigious_empty_recursively_high(1, order).expect("OOM in physical memory allocator")
    }

    /// This function finds a contigious block of empty pages of the given order
    /// The returned address is always aligned by the order of pages
    /// This function is slow! only use when necessary
    fn find_contigious_empty_recursively_low(&self, curr_index: u64, order: u64) -> Option<u64> {
        if curr_index >= self.binary_tree_size / (1 << (order + 1)) {
            //check all pages in this region
            let start_index = curr_index * (1 << order);
            let end_index = start_index + (1 << order);
            for i in start_index..end_index {
                if self.get_at_index(i) {
                    return None;
                }
            }
            return Some(start_index);
        }
        if !self.get_at_index(curr_index * 2) {
            let res = self.find_contigious_empty_recursively_low(curr_index * 2, order);
            if res.is_some() {
                return res;
            }
            self.find_contigious_empty_recursively_low(curr_index * 2 + 1, order)
        } else {
            self.find_contigious_empty_recursively_low(curr_index * 2 + 1, order)
        }
    }

    fn find_contigious_empty_recursively_high(&self, curr_index: u64, order: u64) -> Option<u64> {
        if curr_index >= self.binary_tree_size / (1 << (order + 1)) {
            //check all pages in this region
            let start_index = curr_index * (1 << order);
            let end_index = start_index + (1 << order);
            for i in start_index..end_index {
                if self.get_at_index(i) {
                    return None;
                }
            }
            return Some(start_index);
        }
        if !self.get_at_index(curr_index * 2 + 1) {
            let res = self.find_contigious_empty_recursively_high(curr_index * 2 + 1, order);
            if res.is_some() {
                return res;
            }
            self.find_contigious_empty_recursively_high(curr_index * 2, order)
        } else {
            self.find_contigious_empty_recursively_high(curr_index * 2, order)
        }
    }

    fn mark_addr(&self, addr: PhysAddr, allocated: bool) {
        #[cfg(debug_assertions)]
        assert!(
            addr.0 & 0xFFF == 0,
            "error in mark_addr at addr {} and allocated {}: address is not page aligned",
            addr.0,
            allocated
        );
        self.mark_index((addr.0 >> 12) + (self.binary_tree_size / 2), allocated);
    }

    fn mark_index(&self, index: u64, allocated: bool) {
        self.set_at_index(index, allocated);
        self.update_from_lower(index >> 1);
        self.update_from_higher(index << 1, allocated);
    }

    fn update_from_lower(&self, index: u64) {
        if index == 0 {
            return;
        }
        let all_allocated = self.get_at_index(index << 1) && self.get_at_index((index << 1) + 1);
        self.set_at_index(index, all_allocated);
        self.update_from_lower(index >> 1);
    }

    //should only be used when a page or group of pages is set, so all sub-fragments are also
    //marked as used/unused
    fn update_from_higher(&self, index: u64, allocated: bool) {
        if index >= self.n_pages + self.binary_tree_size / 2 {
            //out of bounds
            return;
        }
        self.set_at_index(index, allocated);

        self.update_from_higher(index << 1, allocated);
        self.update_from_higher((index << 1) + 1, allocated);
    }

    fn get_at_index(&self, index: u64) -> bool {
        unsafe {
            let num = *get_at_virtual_addr::<u8>(self.tree_allocator + (index >> 3));
            num & (1 << (index & 0b111)) > 0
        }
    }

    fn set_at_index(&self, index: u64, allocated: bool) {
        //still ugly code
        unsafe {
            let mut num = *get_at_virtual_addr::<u8>(self.tree_allocator + (index >> 3));
            match allocated {
                true => {
                    num |= 1 << (index & 0b111);
                }
                false => {
                    num &= !(1 << (index & 0b111));
                }
            }
            set_at_virtual_addr(self.tree_allocator + (index >> 3), num);
        }
    }

    fn update_all(&self) {
        for i in (1..self.binary_tree_size / 2).rev() {
            self.set_at_index(i, self.get_at_index(i << 1) && self.get_at_index((i << 1) + 1));
        }
    }
}

fn find_mem_region_to_shrink(memory_regions: &[&mut limine::MemoryMapEntry], space_needed_bytes: u64) -> usize {
    //we search from the last region, to preserve memory for smp code
    for region in memory_regions.iter().enumerate().rev() {
        if !is_memory_region_usable(region.1) {
            continue;
        }
        let empty_space = region.1.length;
        if empty_space >= space_needed_bytes {
            return region.0;
        }
    }

    panic!("Not enough ram for physical allocator")
}

fn find_max_ram_address(memory_regions: &[&mut limine::MemoryMapEntry]) -> PhysAddr {
    let mut highest = 0;
    for region in memory_regions {
        if can_mem_region_be_usable(region) {
            highest = region.base + region.length;
        }
    }
    PhysAddr(highest)
}

fn is_memory_region_usable(entry: &limine::MemoryMapEntry) -> bool {
    entry.entry_type == limine::LIMINE_MEMMAP_USABLE
}

///This function returns if a region can EVER be usable, even if it's currently not (includes
///bootloader reclaimable memory)
fn can_mem_region_be_usable(entry: &limine::MemoryMapEntry) -> bool {
    entry.entry_type == limine::LIMINE_MEMMAP_USABLE || entry.entry_type == limine::LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE
}

///rounded up to power of 2
fn get_binary_tree_element_cnt(n_pages: u64) -> u64 {
    let power = log2_rounded_up(n_pages);
    1 << power
}
