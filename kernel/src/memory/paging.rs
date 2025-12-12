use core::fmt::Display;
use std::{error::ErrorCode, print, println, string::String};

use bitfield::bitfield;

use crate::memory::physical_allocator::is_on_ram;

use super::{mem_utils::*, physical_allocator};

bitfield! {
    #[derive(Clone, Copy)]
    pub struct PageTableEntry(u64);
    pub present, set_present: 0;
    pub writeable, set_writeable: 1;
    pub user_accessible, set_user_accessible: 2;
    pub page_write_through, set_page_write_through: 3;
    pub page_cache_disable, set_page_cache_disable: 4;
    pub accessed, _: 5;
    pub dirty, _: 6;
    pub huge_page, set_huge_page: 7; //is shared with pat
    pub global, set_global: 8;
    pub reserved, _: 51, 48;
    pub no_execute, set_no_execute: 63;
}

impl Display for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PageTableEntry")
            .field(&format_args!("P({})", self.0 & 0b1))
            .field(&format_args!("R/W({})", self.writeable()))
            .field(&format_args!("U({})", self.user_accessible()))
            .field(&format_args!("RSVD({})", self.reserved()))
            .finish()
    }
}

//first 4 are identical as at power-on/reset
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiminePat {
    WB = 0,
    WT = 1,
    UCMinus = 2,
    UC = 3,
    WP = 4,
    WC = 5,
}

impl PageTableEntry {
    //creates default entry:
    //present, writeable, not user accessible, not write-through, not cache disabled, not accessed,
    //not dirty, not huge, not global
    pub fn new(phys_address: PhysAddr, user_mode: bool) -> Self {
        let mut entry = Self((phys_address.0 & 0x_FFF_FFF_FFF_000) | 0b000000011 | (1 << 63));
        if user_mode {
            entry.0 |= 4;
        }
        entry.set_num_of_available_pages(512);
        entry
    }

    pub fn address(&self) -> PhysAddr {
        PhysAddr(self.0 & 0xF_FFF_FFF_FFF_000)
    }

    pub fn set_address(&mut self, address: PhysAddr) {
        const MASK: u64 = 0xF_FFF_FFF_FFF_000;
        self.0 = (self.0 & !MASK) | (address.0 & MASK);
    }

    pub fn set_pat(&mut self, pat_val: LiminePat) {
        let (_pat, pcd, pwt) = match pat_val {
            LiminePat::WB => (false, false, false),
            LiminePat::WT => (false, false, true),
            LiminePat::UCMinus => (false, true, false),
            LiminePat::UC => (false, true, true),
            LiminePat::WP => (true, false, false),
            LiminePat::WC => (true, false, true),
        };
        self.set_page_cache_disable(pcd);
        self.set_page_write_through(pwt);
        //for now i ignore pat teehee :3
        //pat bit depends on if it's a page directory or page table. Can be checked with huge
        //table, but huge-huge tables (1GB) also have huge tables, and don't have pat bit

        PageTree::reload();
    }

    pub fn pat(&self) -> LiminePat {
        let pcd = self.page_cache_disable();
        let pwt = self.page_write_through();

        match (pcd, pwt) {
            (false, false) => LiminePat::WB,
            (false, true) => LiminePat::WT,
            (true, false) => LiminePat::UCMinus,
            (true, true) => LiminePat::UC,
        }
    }

    pub fn num_of_available_pages(&self) -> u64 {
        (self.0 >> 52) & 0b1111111111
    }

    fn set_num_of_available_pages(&mut self, num: u64) {
        const MASK: u64 = !(0b11111111111_u64 << 52);
        const NUM_MASK: u64 = !MASK;
        self.0 = (self.0 & MASK) | ((num << 52) & NUM_MASK);
    }

    pub fn decrease_available(&mut self) {
        self.0 -= 1 << 52;
    }

    pub fn increase_available(&mut self) {
        self.0 += 1 << 52;
    }
}

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Format the output
        f.debug_struct("PageTableEntry")
            .field("present", &self.present())
            .field("address", &self.address())
            .field("available pages", &self.num_of_available_pages())
            .field("huge page", &self.huge_page())
            .field("no execute", &self.no_execute())
            .field("writeable", &self.writeable())
            .field("write through", &self.page_write_through())
            .field("disable cache", &self.page_cache_disable())
            .field("user accessible", &self.user_accessible())
            .field("accessed", &self.accessed())
            .field("dirty", &self.dirty())
            .field("global", &self.global())
            .finish()
    }
}

#[repr(align(4096))]
#[derive(Debug)]
struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = PageTableEntry(0);
        }
    }

    fn print_range(&self, mut current_range: Option<MapRange>, level: u64, mut self_virt_addr: VirtAddr) -> Option<MapRange> {
        for entry in self.entries {
            if !entry.present() {
                if let Some(range) = &current_range {
                    println!("{range}");
                    current_range = None;
                }
                self_virt_addr += 1 << (3 + level * 9);
                continue;
            }
            if level == 1 || entry.huge_page() {
                if let Some(curr_range) = current_range.clone() {
                    if curr_range.pat != entry.pat()
                        || curr_range.write != entry.writeable()
                        || curr_range.execute == entry.no_execute()
                        || (curr_range.phys.0 + curr_range.len != entry.address().0
                            && curr_range.phys.0 - 0x1000 != entry.address().0)
                    {
                        println!("{curr_range}");
                        current_range = None
                    }
                }

                if let Some(curr_range) = current_range.clone() {
                    let new_range = MapRange {
                        len: curr_range.len + (1 << (3 + level * 9)),
                        phys: PhysAddr(curr_range.phys.0.min(entry.address().0)),
                        ..curr_range
                    };
                    current_range = Some(new_range);
                } else {
                    let new_range = MapRange {
                        virt: self_virt_addr,
                        len: 1 << (3 + level * 9),
                        phys: entry.address(),
                        pat: entry.pat(),
                        write: entry.writeable(),
                        execute: !entry.no_execute(),
                        user: entry.user_accessible(),
                    };
                    current_range = Some(new_range);
                }
            } else {
                let lower_level_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
                let new_range = lower_level_table.print_range(current_range.clone(), level - 1, self_virt_addr);
                current_range = new_range;
            }
            self_virt_addr += 1 << (3 + level * 9);
        }
        current_range
    }

    pub fn print_entry_bitmask(&self) {
        for chunk in self.entries.chunks(128) {
            for entry in chunk {
                print!("{}", if entry.present() { 1 } else { 0 });
            }
            println!("");
        }
    }

    pub fn print_entries(&self, addr: VirtAddr, level: u64) {
        let entry_inedx = (addr.0 >> (3 + level * 9)) & 0b111_111_111;
        let entry = &self.entries[entry_inedx as usize];
        if entry.present() {
            println!("Entry on level {level}: {:X}, {}", entry.0, entry);
            if level != 1 && !entry.huge_page() {
                let lower_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
                lower_table.print_entries(addr, level - 1);
            }
        } else {
            println!("Entry at {:#x?} on level {level} is not present", addr.0 >> (3 + level * 9));
        }
    }

    fn unmap_lower_half(&mut self) {
        let entris_to_remove = &mut self.entries[..256];
        for entry in entris_to_remove {
            entry.set_present(false);
        }
    }

    ///prepares a level 3 table for each of the higher half addresses, so these tables can be
    ///shared between processes
    fn prepare_higher_half(&mut self) {
        for entry in &mut self.entries[256..] {
            if entry.present() {
                continue;
            }
            let frame = physical_allocator::allocate_frame_low();
            let table = unsafe { get_at_physical_addr::<PageTable>(frame) };
            table.clear();
            *entry = PageTableEntry::new(frame, false);
        }

        //remove highest page from pool
        let highest_virt_page = VirtAddr(0xFFFF_FFFF_FFFF_F000);
        unsafe { self.mmap(highest_virt_page, PhysAddr(0)) };
        let entry = self.get_page_table_entry(highest_virt_page, 4).expect("page entry must exist after allocation");
        entry.set_writeable(false);
        entry.set_no_execute(true);
    }

    ///returns entry index at which a page is available. If no such address exists, it panics
    pub fn get_available_entry(&self, low: bool) -> usize {
        if low {
            for entry in self.entries.iter().enumerate() {
                if !entry.1.present() || (!entry.1.huge_page() && entry.1.num_of_available_pages() > 0) {
                    return entry.0;
                }
            }
        } else {
            for entry in self.entries.iter().enumerate().rev() {
                if !entry.1.present() || (!entry.1.huge_page() && entry.1.num_of_available_pages() > 0) {
                    return entry.0;
                }
            }
        }
        panic!("could not find available virtual page");
    }

    ///returns the address after which the requested number of pages are available.
    ///If such an address doesn't exist the function panics, because in reality that's pretty much
    ///impossible. Computers will run out of memory before that happens
    ///# Safety
    ///This function must be called with a valid level. External callers should always use 4
    ///number of pages requested cannot be more than 512
    unsafe fn get_available_entry_pages(&self, level: u64, pages: u64, low: bool) -> Option<u64> {
        debug_assert!(pages <= 512);

        if low {
            for iterator in self.entries.iter().enumerate() {
                if let Some(addr) = internal(iterator, level, pages, low) {
                    return Some(addr);
                }
            }
        } else {
            for iterator in self.entries.iter().enumerate().rev() {
                if let Some(addr) = internal(iterator, level, pages, low) {
                    return Some(addr);
                }
            }
        };

        fn internal(entry: (usize, &PageTableEntry), level: u64, pages: u64, low: bool) -> Option<u64> {
            if !entry.1.present() {
                return Some((entry.0 as u64) << (3 + level * 9));
            }
            if entry.1.present() && (!entry.1.huge_page() && (level > 1 || entry.1.num_of_available_pages() >= pages)) {
                let lower_table = unsafe { get_at_physical_addr::<PageTable>(entry.1.address()) };
                let addr = if level == 2 {
                    lower_table.get_available_entry_level_1_pages(pages, low)?
                } else {
                    unsafe { lower_table.get_available_entry_pages(level - 1, pages, low)? }
                };
                return Some(((entry.0 as u64) << (3 + level * 9)) + addr);
            }
            None
        }
        return None;
    }

    fn get_available_entry_level_1(&self, low: bool) -> usize {
        if low {
            for entry in self.entries.iter().enumerate() {
                if !entry.1.present() {
                    return entry.0;
                }
            }
        } else {
            for entry in self.entries.iter().enumerate().rev() {
                if !entry.1.present() {
                    return entry.0;
                }
            }
        }
        panic!("didn't run out of memory, but still ran out of pages????");
    }

    ///returns the address after which the requested number of pages are available.
    fn get_available_entry_level_1_pages(&self, pages: u64, low: bool) -> Option<u64> {
        if low {
            for entries in self.entries.windows(pages as usize).enumerate() {
                if entries.1.iter().all(|entry| !entry.present()) {
                    return Some((entries.0 as u64) << 12);
                }
            }
        } else {
            for entries in self.entries.windows(pages as usize).enumerate().rev() {
                if entries.1.iter().all(|entry| !entry.present()) {
                    return Some((entries.0 as u64) << 12);
                }
            }
        }
        return None
    }

    pub fn allocate(&mut self, virtual_address: VirtAddr) -> Result<PhysAddr, ErrorCode> {
        unsafe {
            let frame_addr = physical_allocator::allocate_frame();
            if let Err(e) = self.mmap(virtual_address, frame_addr) {
                physical_allocator::deallocate_frame(frame_addr);
                return Err(e);
            }
            Ok(frame_addr)
        }
    }

    pub fn allocate_any(&mut self, low: bool) -> VirtAddr {
        unsafe {
            let frame_addr = physical_allocator::allocate_frame();
            self.mmap_any(frame_addr, low)
        }
    }

    ///maps some available virtual address to the given physical address. Physical address must be
    ///marked as used
    ///# Safety
    ///Physical address must be marked as used by an external actor
    pub unsafe fn mmap_any(&mut self, physical_address: PhysAddr, low: bool) -> VirtAddr {
        debug_assert!(!is_on_ram(physical_address) || physical_allocator::is_frame_allocated(physical_address));
        let mut address = 0;
        unsafe { self.allocate_4_to_2(4, &mut address, physical_address, low) };
        if address & (1 << 47) != 0 {
            address += 0xFFFF << 48; //sign extension
        }
        VirtAddr(address)
    }

    ///# Seafety
    ///Physical addresses from physical_address to physical_address + num must already be marked as
    ///used, and not yet mapped
    pub unsafe fn mmap_contigious_any(&mut self, num: u64, physical_address: PhysAddr, low: bool) -> VirtAddr {
        let address = unsafe { self.get_available_entry_pages(4, num, low).ok_or(ErrorCode::InsufficientResources).expect("no pages?? impossible!") };
        for i in 0..num {
            assert!(
                !is_on_ram(physical_address + PhysAddr(i * 4096))
                    || physical_allocator::is_frame_allocated(physical_address + PhysAddr(i * 4096)),
                "memory space must already be allocated"
            );
            unsafe {
                self.mmap(VirtAddr(address + i * 4096), physical_address + PhysAddr(i * 4096)).expect("inconsistency between searching free contigious pages and allocating them in mmap_contigious_any");
            }
        }
        VirtAddr(address)
    }

    ///maps the given virtual address to the given physical address. Physical address must be
    ///marked as used
    ///# Safety
    ///physical address must be marked as used by an external actor. Virtual address must
    ///not yet be in use by this page tree
    pub unsafe fn mmap(&mut self, virtual_address: VirtAddr, physical_address: PhysAddr) -> Result<(), ErrorCode> {
        debug_assert!(!is_on_ram(physical_address) || physical_allocator::is_frame_allocated(physical_address));
        unsafe {
            self.allocate_4_to_2_virtual(4, virtual_address, physical_address).map(|_| ())
        }
    }

    ///returns if that page table has less available spaces
    unsafe fn allocate_4_to_2(&mut self, level: u64, address: &mut u64, physical_address: PhysAddr, low: bool) -> bool {
        let index_of_available = self.get_available_entry(low);

        *address += (index_of_available as u64) << (3 + level * 9);
        let entry = &mut self.entries[index_of_available];

        if !entry.present() {
            let frame_addr = physical_allocator::allocate_frame();
            let page_table = unsafe { get_at_physical_addr::<PageTable>(frame_addr) };
            page_table.clear();
            let temp_entry = PageTableEntry::new(frame_addr, is_user_mode(*address));
            *entry = temp_entry;
        }

        let lower_page_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
        let lower_less_available = if level == 2 {
            unsafe { lower_page_table.allocate_level_1(address, physical_address, low) };
            true
        } else {
            unsafe { lower_page_table.allocate_4_to_2(level - 1, address, physical_address, low) }
        };
        if lower_less_available {
            entry.decrease_available();
        }
        entry.num_of_available_pages() == 0
    }

    ///#Safety: Virtual address must not yet be in use by this page tree. Physical address must be
    ///marked as used
    unsafe fn allocate_4_to_2_virtual(&mut self, level: u64, address: VirtAddr, physical_address: PhysAddr) -> Result<bool, ErrorCode> {
        let entry = self.get_page_table_entry_on_level(address, level);
        if !entry.present() {
            let frame_addr = physical_allocator::allocate_frame();
            let page_table = unsafe { get_at_physical_addr::<PageTable>(frame_addr) };
            page_table.clear();
            let temp_entry = PageTableEntry::new(frame_addr, is_user_mode(address));
            *entry = temp_entry;
            debug_assert_eq!(temp_entry.0, entry.0);
        }

        let lower_less_available = unsafe {
            let lower_page_table = get_at_physical_addr::<PageTable>(entry.address());
            if level == 2 {
                lower_page_table.allocate_level_1_virtual(address, physical_address)?;
                true
            } else {
                lower_page_table.allocate_4_to_2_virtual(level - 1, address, physical_address)?
            }
        };
        if lower_less_available {
            entry.decrease_available();
        }
        Ok(entry.num_of_available_pages() == 0)
    }

    ///# Safety
    ///This level MUST have empty address slot. This must be ensured by higher levels
    unsafe fn allocate_level_1(&mut self, address: &mut u64, physical_address: PhysAddr, low: bool) {
        let index_of_available = self.get_available_entry_level_1(low);
        *address += (index_of_available as u64) << 12;
        let entry = &mut self.entries[index_of_available];

        *entry = PageTableEntry::new(physical_address, is_user_mode(*address));
    }

    unsafe fn allocate_level_1_virtual(&mut self, virtual_address: VirtAddr, physical_address: PhysAddr) -> Result<(), ErrorCode> {
        let entry = self.get_page_table_entry_on_level(virtual_address, 1);
        if entry.present() {
            return Err(ErrorCode::AlreadyMapped);
        }

        *entry = PageTableEntry::new(physical_address, is_user_mode(virtual_address));
        Ok(())
    }

    //returns if there was no space but now there is
    pub unsafe fn deallocate(&mut self, address: VirtAddr, level: u64) -> bool {
        let entry = &mut self.entries[(address.0 >> (3 + level * 9) & 0b111_111_111) as usize];
        debug_assert!(entry.present());
        if level == 1 {
            entry.set_present(false);
            unsafe { physical_allocator::deallocate_frame(entry.address()) };
            return true;
        }
        if entry.present() && entry.huge_page() {
            entry.set_present(false);
            dealloc_huge_page(entry, level);
            return true;
        }
        unsafe {
            let lower_level_table = get_at_physical_addr::<PageTable>(entry.address());
            let more_space = lower_level_table.deallocate(address, level - 1);
            if !more_space {
                return false;
            }
        }
        entry.increase_available();
        //if it was 0 before, this entry was not available but now it is
        entry.num_of_available_pages() == 1
    }

    //TODO: when unmapping lowest pages, also unmap higher pages

    //returns if there was no space but now there is
    pub unsafe fn unmap(&mut self, address: VirtAddr, level: u64) -> Result<bool, ErrorCode> {
        let entry = &mut self.entries[(address.0 >> (3 + level * 9) & 0b111_111_111) as usize];
        if !entry.present() {
            return Err(ErrorCode::NotMapped)
        }
        if level == 1 {
            entry.set_present(false);
            return Ok(true);
        }
        if entry.huge_page() {
            entry.set_present(false);
            return Ok(true);
        }
        unsafe {
            let lower_level_table = get_at_physical_addr::<PageTable>(entry.address());
            let more_space = lower_level_table.unmap(address, level - 1)?;
            if !more_space {
                return Ok(false);
            }
        }
        entry.increase_available();
        //if it was 0 before, this entry was not available but now it is
        Ok(entry.num_of_available_pages() == 1)
    }

    ///init function
    pub fn set_num_of_available_spaces(&mut self, level: u64) -> u64 {
        let mut sum = 0;
        for entry in self.entries.iter_mut() {
            if !entry.present() {
                sum += 1;
                continue;
            }
            if level == 1 || entry.huge_page() {
                continue;
            }
            unsafe {
                let lower_level_page = get_at_physical_addr::<PageTable>(entry.address());
                let lower_available = lower_level_page.set_num_of_available_spaces(level - 1);
                entry.set_num_of_available_pages(lower_available);
                if lower_available > 0 {
                    sum += 1;
                }
            }
        }

        sum
    }

    fn get_num_allocated_spaces(&self, level: u64) -> u64 {
        let mut sum = 0;

        for entry in self.entries.iter() {
            if !entry.present() {
                continue;
            }
            if level == 1 || entry.huge_page() {
                sum += 1;
                continue;
            }
            unsafe {
                let lower_level_page = get_at_physical_addr::<PageTable>(entry.address());
                let lower_available = lower_level_page.get_num_allocated_spaces(level - 1);
                sum += lower_available;
            }
        }
        sum
    }

    fn get_page_table_entry(&mut self, addr: VirtAddr, level: u64) -> Option<&mut PageTableEntry> {
        unsafe {
            let entry = (addr.0 >> (12 + 9 * (level - 1))) & 0x1FF;
            let entry = &mut self.entries[entry as usize];
            if !entry.present() {
                return None;
            }
            if level == 1 || entry.huge_page() {
                Some(entry)
            } else {
                let lower_table = get_at_physical_addr::<PageTable>(entry.address());
                lower_table.get_page_table_entry(addr, level - 1)
            }
        }
    }

    #[inline]
    fn get_page_table_entry_on_level(&mut self, addr: VirtAddr, level: u64) -> &mut PageTableEntry {
        let entry = (addr.0 >> (12 + 9 * (level - 1))) & 0x1FF;
        &mut self.entries[entry as usize]
    }

    fn set_execute_recursive(&mut self, addr: VirtAddr, level: u64) {
        let entry = self.get_page_table_entry_on_level(addr, level);
        debug_assert!(entry.present());
        entry.set_no_execute(false);
        if level == 1 || entry.huge_page() {
            return;
        }
        let lower_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
        lower_table.set_execute_recursive(addr, level - 1);
    }
}

fn dealloc_huge_page(entry: &PageTableEntry, level: u64) {
    #[cfg(debug_assertions)]
    assert!(level == 2 || level == 3);

    let physical_address = entry.address();
    let num_to_dealloc = 512 * if level == 3 { 512 } else { 1 };
    for j in 0..num_to_dealloc {
        unsafe {
            physical_allocator::deallocate_frame(physical_address + PhysAddr(j * 4096));
        }
    }
}

#[derive(Debug)]
pub struct PageTree {
    level_4_table: PhysAddr,
}

impl PageTree {
    pub const fn new(level_4_table: PhysAddr) -> Self {
        Self { level_4_table }
    }

    pub fn root(&self) -> PhysAddr {
        self.level_4_table
    }

    pub fn print_mapping(&self) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            if let Some(range) = level_4_table.print_range(None, 4, VirtAddr(0)) {
                println!("{range}");
            }
        }
    }

    pub fn print_entries(&self, addr: VirtAddr) {
        println!("printing entries for {:?}", addr);
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            level_4_table.print_entries(addr, 4);
        }
    }

    ///This function walks the page table and sets the number of available spaces in the lower
    ///level pages. It also maps highest addr as user inaccessible, not writable and not executable.
    ///Kernel can still read, but by mapping it to physical address 0 and not using it it's fine
    pub fn init(&mut self) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);

            level_4_table.prepare_higher_half();
            level_4_table.set_num_of_available_spaces(4);
        }
        PageTree::reload();
    }

    pub fn unmap_lower_half(&mut self) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            level_4_table.unmap_lower_half();
        }
    }

    pub fn get_num_allocated_pages(&self) -> u64 {
        let level_4_table = unsafe { get_at_physical_addr::<PageTable>(self.level_4_table) };
        level_4_table.get_num_allocated_spaces(4)
    }

    pub fn get_level4_addr() -> PhysAddr {
        let mut level_4_table = PhysAddr(0);
        unsafe {
            core::arch::asm!(
                "mov {}, cr3",
                out(reg) level_4_table.0,
            );
        }
        level_4_table
    }

    pub fn set_level4_addr(level_4_table: PhysAddr) {
        unsafe {
            core::arch::asm!(
                "mov cr3, {}",
                in(reg) level_4_table.0,
            );
        }
    }

    pub fn reload() {
        unsafe {
            let level_4_table = Self::get_level4_addr();
            core::arch::asm!(
                "mov cr3, {}",
                in(reg) level_4_table.0,
            );
        }
    }

    pub fn copy_higher_half(&self, new_page_tree: &mut PageTree) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            let new_level_4_table = get_at_physical_addr::<PageTable>(new_page_tree.level_4_table);
            for i in 256..512 {
                new_level_4_table.entries[i] = level_4_table.entries[i];
            }
        }
    }

    pub fn unmap_higher_half(&mut self) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            for i in 256..512 {
                let entry = &mut level_4_table.entries[i];
                entry.set_present(false);
            }
        }
    }

    pub fn set_execute(&mut self, addr: VirtAddr) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            level_4_table.set_execute_recursive(addr, 4);
        }
    }
}

//public API
impl PageTree {
    pub fn allocate(&mut self, physical_address: Option<PhysAddr>, low: bool) -> std::mem_utils::VirtAddr {
        //TODO:, make mmap and such methods here instead of Options
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            let address = match physical_address {
                None => level_4_table.allocate_any(low),
                Some(physical_address) => level_4_table.mmap_any(physical_address, low),
            };
            if low {
                return address;
            }

            //force sign extension
            VirtAddr(address.0 | 0xFFFF_0000_0000_0000)
        }
    }

    pub fn allocate_set_virtual(
        &mut self,
        physical_address: Option<PhysAddr>,
        virtual_address: std::mem_utils::VirtAddr,
    ) -> Result<PhysAddr, ErrorCode> {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            match physical_address {
                None => level_4_table.allocate(virtual_address),
                Some(physical_address) => {
                    level_4_table.mmap(virtual_address, physical_address)?;
                    Ok(physical_address)
                }
            }
        }
    }

    pub fn deallocate(&mut self, addr: std::mem_utils::VirtAddr) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            level_4_table.deallocate(addr, 4);
        }
    }

    pub fn unmap(&mut self, addr: std::mem_utils::VirtAddr) {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            level_4_table.unmap(addr, 4);
        }
    }

    pub fn allocate_contigious(&mut self, num: u64, physical_address: Option<PhysAddr>, low: bool) -> VirtAddr {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            let address = match physical_address {
                None => {
                    let addr = level_4_table.get_available_entry_pages(4, num, low).ok_or(ErrorCode::InsufficientResources).expect("no pages?? impossible!");
                    for i in 0..num {
                        level_4_table.allocate(VirtAddr(addr + i * 4096)).expect("inconsistency between searching free contigious pages and allocating them in mmap_contigious_any");
                    }
                    VirtAddr(addr)
                }
                Some(physical_address) => level_4_table.mmap_contigious_any(num, physical_address, low),
            };
            if low {
                return address;
            }
            //force sign extension
            VirtAddr(address.0 | 0xFFFF_0000_0000_0000)
        }
    }

    pub fn mmap_contigious(&mut self, physical_addresses: &[PhysAddr], low: bool) -> VirtAddr {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            let addr = level_4_table.get_available_entry_pages(4, physical_addresses.len() as u64, low).expect("no pages?? impossible!");
            for i in 0..physical_addresses.len() {
                level_4_table.mmap(VirtAddr(addr + i as u64 * 4096), physical_addresses[i]).expect("inconsistency between searching free contigious pages and allocating them in mmap_contigious_any");
            }
            if low {
                return VirtAddr(addr);
            }
            //force sign extension
            VirtAddr(addr | 0xFFFF_0000_0000_0000)
        }
    }

    pub fn find_contigious_pages(&mut self, n_pages: usize, low: bool) -> Option<std::mem_utils::VirtAddr> {
        let level_4_table = unsafe { get_at_physical_addr::<PageTable>(self.level_4_table) };
        let addr = unsafe { VirtAddr(level_4_table.get_available_entry_pages(4, n_pages as u64, low)?) };
        if low {
            return Some(addr);
        }
        //force sign extension
        Some(VirtAddr(addr.0 | 0xFFFF_0000_0000_0000))
    }

    pub fn get_page_table_entry_mut(&mut self, addr: std::mem_utils::VirtAddr) -> Option<&mut PageTableEntry> {
        unsafe {
            let level_4_table = get_at_physical_addr::<PageTable>(self.level_4_table);
            level_4_table.get_page_table_entry(VirtAddr(addr.0 & !0xFFF), 4)
        }
    }
}

pub fn is_user_mode<T: Into<u64>>(addr: T) -> bool {
    let _u64: u64 = addr.into();
    _u64 < 0x800000000000
}

#[derive(Debug, Clone)]
pub struct MapRange {
    pub virt: VirtAddr,
    pub phys: PhysAddr,
    pub len: u64,
    pub pat: LiminePat,
    pub write: bool,
    pub execute: bool,
    pub user: bool,
}

impl Display for MapRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut rwxu = String::from("r");
        if self.write {
            rwxu.push('w');
        } else {
            rwxu.push('-');
        }
        if self.execute {
            rwxu.push('x');
        } else {
            rwxu.push('-');
        }
        if self.user {
            rwxu.push('u');
        } else {
            rwxu.push('k');
        }
        let addr_start = if self.virt.0 & (1 << 47) != 0 {
            self.virt.0 + (0xFFFF << 48)
        } else {
            self.virt.0
        };
        let addr_end = if self.virt.0 & (1 << 47) != 0 {
            (self.virt.0 + (0xFFFF << 48)).wrapping_add(self.len)
        } else {
            self.virt.0 + self.len
        };
        write!(
            f,
            "Range: virt: {:016x}, end: {:016x}, phys start: {:016x}, pat: {:?}, rwx: {:?}",
            addr_start, addr_end, self.phys.0, self.pat, rwxu
        )
    }
}
