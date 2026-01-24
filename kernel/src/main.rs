#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]
#![feature(stmt_expr_attributes)]
#![feature(box_into_inner)]
#![feature(string_remove_matches)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]
#![feature(c_str_module)]
#![feature(str_from_raw_parts)]
#![feature(slice_index_methods)]
#![feature(new_range_api)]
#![feature(rustc_attrs)]
#![feature(unsafe_cell_access)]
#![allow(internal_features)]

extern crate static_cond;

use core::ffi;
use std::{boxed::Box, println, printlnc};

mod acpi;
mod clocks;
mod cmd_args;
mod cpuid;
mod drivers;
mod file_operations;
mod interrupts;
mod keyboard;
mod limine;
mod memory;
mod msr;
mod parsers;
mod proc;
mod task_runner;
#[allow(unused_imports)]
mod tests;
mod utils;
mod vfs;
mod vga;
mod printer;
mod rand;
use limine::LIMINE_BOOTLOADER_REQUESTS;
use task_runner::block_task;
use vfs::ResolvedPath;

const PRIME_FINDER: &[u8] = include_bytes!("../../assets/prime_finder");
const TIME_PRINTER: &[u8] = include_bytes!("../../assets/time_printer");
const FILE_READER: &[u8] = include_bytes!("../../assets/file_reader");

#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    let stack_pointer: *const u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) stack_pointer);
    }

    rand::init_rand();

    vga::init_vga_driver();
    printer::init_printer();
    vga::clear_screen();

    let cmd_line_info = unsafe { &(*LIMINE_BOOTLOADER_REQUESTS.cmd_line_request.info) };
    let str = unsafe { ffi::CStr::from_ptr(cmd_line_info.cmdline) };

    println!("starting RustOs...");
    println!("stack pointer: {:?}", stack_pointer);

    memory::init_memory();

    acpi::cpu_locals::init_dummy_cpu_locals();

    interrupts::init_interrupts();

    let cmd_args = cmd_args::CmdArgs::new(str.to_str().expect("Invalid UTF-8 in cmdline"));
    println!("cmd_args: {:?}", cmd_args);

    acpi::read_tables();

    clocks::init();

    acpi::init_acpi();

    // let mut counter = 0;
    //
    // loop {
    //     println!("Counting {}", counter);
    //     counter += 1;
    //     let now = std::time::Instant::now();
    //     while std::time::Instant::now() - now < core::time::Duration::from_secs(1) {}
    // }

    drivers::init_drivers();

    drivers::pci::init();

    std::thread::sleep(core::time::Duration::from_micros(11));

    vfs::init();

    let res = block_task(Box::pin(vfs::mount_blkdev_partition(
        cmd_args.root_partition,
        ResolvedPath::root(),
    )));
    if let Err(e) = res {
        println!("{}", e);
        panic!("Failed to mount root partition");
    }
    //
    // let path = vfs::resolve_path("/");
    // let file_open_flags = FileFlags::new_with_flags(true, false, false, false);
    // let file = block_task(Box::pin(vfs::open_file((&path).into(), None, file_open_flags))).unwrap();
    // println!("{:?}", block_task(Box::pin(vfs::get_dir_entries(&file))));
    // file_operations::do_file_operations();
    //
    // loop {
    //     process_tasks();
    // }

    proc::init();

    //start first proc
    unsafe { core::arch::asm!("int 254") };

    panic!("Returned to _start after first context switch");
}
