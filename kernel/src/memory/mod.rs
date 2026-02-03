pub mod heap;
pub mod paging;
pub mod physical_allocator;
pub mod stack;

use crate::LIMINE_BOOTLOADER_REQUESTS;
use crate::{println, printlnc};
use std::mem_utils::{self, PhysAddr};

pub static mut PAGE_TREE_ALLOCATOR: paging::PageTree = paging::PageTree::new(PhysAddr(0));

pub static mut TRAMPOLINE_RESERVED: PhysAddr = PhysAddr(0);

pub fn init_memory() {
    println!(level:info, "initializing memory");
    print_limine_phys_map();
    unsafe {
        let offset: u64 = (*LIMINE_BOOTLOADER_REQUESTS.higher_half_direct_map_request.info).offset;
        let len = get_hhdm_map_len();
        mem_utils::set_hhdm_addr(mem_utils::PhysOffset(offset));
        mem_utils::set_hhdm_len(len);
        println!(level:info, "offset: {:#x?}", offset);
        println!(level:info, "initializing physical allocator");
        physical_allocator::init();
        //allocates low addresses first, so we reserve this for the trampoline
        TRAMPOLINE_RESERVED = physical_allocator::allocate_frame_low();
        println!(level:info, "initializing pager");
        let page_table_root = paging::PageTree::get_level4_addr();
        PAGE_TREE_ALLOCATOR = paging::PageTree::new(page_table_root);
        printlnc!(level:info, (255, 200, 100), "Limine mem map:");
        PAGE_TREE_ALLOCATOR.print_mapping();
        PAGE_TREE_ALLOCATOR.init();
        printlnc!(level:info, (0, 255, 0), "memory initialized");
    }
    std::mem_utils::set_heap_initialized();
}

pub fn print_limine_phys_map() {
    //print limine mmap feature. IS it actually a map?
    printlnc!(level:info, (255, 200, 101), "Limine physical memory map:");
    let mmap = unsafe { &(*LIMINE_BOOTLOADER_REQUESTS.memory_map_request.info) };
    let entries = unsafe { core::slice::from_raw_parts(mmap.memory_map, mmap.memory_map_count as usize) };
    for entry in entries {
        let start = entry.base;
        let end = entry.base + entry.length;
        let mem_type = match entry.entry_type {
            0 => "Usable",
            1 => "Reserved",
            2 => "ACPI Reclaimable",
            3 => "ACPI NVS",
            4 => "Bad Memory",
            5 => "Bootloader Reclaimable",
            6 => "Kernel and Modules",
            7 => "Framebuffer",
            8 => "Acpi tables",
            _ => "Unknown",
        };
        println!(level:info, "{:#x?} - {:#x?} ({})", start, end, mem_type);
    }
}

pub fn get_hhdm_map_len() -> u64 {
    let mmap = unsafe { &(*LIMINE_BOOTLOADER_REQUESTS.memory_map_request.info) };
    let entries = unsafe { core::slice::from_raw_parts(mmap.memory_map, mmap.memory_map_count as usize) };
    entries.iter().filter(|entry| {
        entry.entry_type != 1 && entry.entry_type != 4
    }).fold(0u64, |acc, entry| {
        acc.max(entry.base + entry.length)
    })
}
