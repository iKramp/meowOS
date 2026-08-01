pub mod addresses;
mod heap;
pub mod physical_allocator;
pub mod stack;
mod virt_mem_manager;
pub use virt_mem_manager::*;

use crate::LIMINE_BOOTLOADER_REQUESTS;
use crate::interrupts::{disable_interrupts, enable_interrupts};
use crate::{println, printlnc};
use addresses::*;

const LIMINE_STACK_SIZE_PAGES: usize = 16; //64kb stack

pub static mut TRAMPOLINE_RESERVED: OwnedPhysAddr = OwnedPhysAddr(PhysAddr(0));

#[repr(C)]
pub struct ProbeResult {
    value: u64,
    valid: bool,
}

#[link(name = "probe", kind = "static")]
unsafe extern "C" {
    pub static probe_functions_start: u8;
    fn probe_check_u64(ptr: u64) -> ProbeResult;
    fn probe_check_u32(ptr: u64) -> ProbeResult;
    fn probe_check_u16(ptr: u64) -> ProbeResult;
    fn probe_check_u8(ptr: u64) -> ProbeResult;
    fn probe_memcpy(dst: u64, src: u64, len: u64) -> ProbeResult;
    pub fn probe_fail() -> ProbeResult;
    pub static probe_functions_end: u8;
}

pub fn init_memory(stack_start: VirtAddr) {
    fill_stack_with_pattern(stack_start);
    println!(level:info, "initializing memory");
    print_limine_phys_map();
    unsafe {
        let ap_startup_at = crate::acpi::ap_startup as *const () as u64;
        println!(level:info, "ap_startup at {:#x?}", ap_startup_at);
        let offset: u64 = (*LIMINE_BOOTLOADER_REQUESTS.higher_half_direct_map_request.info).offset;
        let len = get_hhdm_map_len();
        set_hhdm_addr(PhysOffset(offset));
        set_hhdm_len(len);
        println!(level:info, "offset: {:#x?}", offset);
        println!(level:info, "initializing physical allocator");
        physical_allocator::init();

        //allocates low addresses first, so we reserve this for the trampoline
        let mut reserved = physical_allocator::reserve_low();
        core::mem::swap(&mut TRAMPOLINE_RESERVED, &mut reserved);
        core::mem::forget(reserved);
        println!(level:info, "initializing pager");
        virt_mem_manager::init_paging();

        printlnc!(level:info, (255, 200, 100), "Limine mem map:");
        virt_mem_manager::print_mem_mapping();
        virt_mem_manager::init_paging();
        printlnc!(level:info, (0, 255, 0), "memory initialized");
    }
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

pub fn fill_stack_with_pattern(stack_start: VirtAddr) {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) rsp);
    }
    let stack_now = (rsp as usize - 0x100) & !0xfff; //align to page boundary
    let stack_start = (stack_start.0 as usize & !0xfff) + 0x1000; //align to page boundary
    let stack_end = stack_start - (LIMINE_STACK_SIZE_PAGES - 1) * 4096; //1 less just in case to not overflow
    let to_fill_len = stack_now - stack_end;

    let stack = unsafe { core::slice::from_raw_parts_mut(stack_end as *mut u8, to_fill_len) };
    let pattern: u8 = 0xAA;

    for byte in stack.iter_mut() {
        *byte = pattern;
    }
}

pub fn get_hhdm_map_len() -> u64 {
    let mmap = unsafe { &(*LIMINE_BOOTLOADER_REQUESTS.memory_map_request.info) };
    let entries = unsafe { core::slice::from_raw_parts(mmap.memory_map, mmap.memory_map_count as usize) };
    entries
        .iter()
        .filter(|entry| entry.entry_type != 1 && entry.entry_type != 4)
        .fold(0u64, |acc, entry| acc.max(entry.base + entry.length))
}

///Performs an exclusive range check for pointers to make sure they're mapped
pub fn probe_pointer_range(ptr_start: u64, mut ptr_end: u64) -> bool {
    ptr_end -= 1; //to make inclusive
    let mut valid = true;
    let first_page = ptr_start & (!0xfff);
    let last_page = ptr_end & (!0xfff);
    let prev_int_state = disable_interrupts();

    for page_addr in first_page..=last_page {
        if !unsafe { probe_check_u64(page_addr) }.valid {
            valid = false;
            break;
        }
    }

    if prev_int_state {
        enable_interrupts();
    }

    if !valid {
        println!(level:warn, "pointer range {:#x?} - {:#x?} is invalid", ptr_start, ptr_end);
    }
    valid
}

pub fn safe_memcpy_from_user(dst: u64, src: u64, len: usize) -> bool {
    let src_end = src + len as u64 - 1;
    if !is_userspace_ptr(VirtAddr(src)) || !is_userspace_ptr(VirtAddr(src_end)) {
        println!(level:warn, "invalid userspace pointer range: {:#x?} - {:#x?}", src, src_end);
        return false;
    }
    safe_memcpy_kernel(dst, src, len)
}

pub fn safe_memcpy_to_user(dst: u64, src: u64, len: usize) -> bool {
    let dst_end = dst + len as u64 - 1;
    if !is_userspace_ptr(VirtAddr(dst)) || !is_userspace_ptr(VirtAddr(dst_end)) {
        println!(level:warn, "invalid userspace pointer range: {:#x?} - {:#x?}", dst, dst_end);
        return false;
    }
    safe_memcpy_kernel(dst, src, len)
}

pub fn safe_memcpy_kernel(dst: u64, src: u64, len: usize) -> bool {
    let prev_int_state = disable_interrupts();

    let res = unsafe { probe_memcpy(dst, src, len as u64) };

    if prev_int_state {
        enable_interrupts();
    }

    res.valid
}

pub fn print_mem_mapping() {
    virt_mem_manager::print_mem_mapping();
}

pub fn probe_ptr_u64(ptr: u64) -> Option<u64> {
    let res = unsafe { probe_check_u64(ptr) };
    if res.valid { Some(res.value) } else { None }
}

pub fn probe_ptr_u32(ptr: u64) -> Option<u32> {
    let res = unsafe { probe_check_u32(ptr) };
    if res.valid { Some(res.value as u32) } else { None }
}

pub fn probe_ptr_u16(ptr: u64) -> Option<u16> {
    let res = unsafe { probe_check_u16(ptr) };
    if res.valid { Some(res.value as u16) } else { None }
}

pub fn probe_ptr_u8(ptr: u64) -> Option<u8> {
    let res = unsafe { probe_check_u8(ptr) };
    if res.valid { Some(res.value as u8) } else { None }
}

pub fn log2_rounded_up(num: u64) -> u64 {
    if num == 1 {
        return 0; //special case
    }
    (num * 2 - 1).ilog2().into()
}
