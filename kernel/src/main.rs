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
#![feature(let_chains)]
#![feature(downcast_unchecked)]
#![feature(map_try_insert)]
#![allow(internal_features)]
#![allow(clippy::fn_to_numeric_cast)]

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
mod net;
use limine::LIMINE_BOOTLOADER_REQUESTS;
use task_runner::block_task;
use vfs::ResolvedPath;

const TIME_PRINTER: &[u8] = include_bytes!("../../assets/time_printer");

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

    println!(level:info, "starting RustOs...");
    println!(level:info, "stack pointer: {:?}", stack_pointer);

    memory::init_memory();

    acpi::cpu_locals::init_dummy_cpu_locals();

    interrupts::init_interrupts();

    let cmd_args = cmd_args::CmdArgs::new(str.to_str().expect("Invalid UTF-8 in cmdline"));
    println!(level:info, "cmd_args: {:?}", cmd_args);

    
    let test_ptr = acpi::read_tables as u64;
    let probe_res = memory::probe_ptr_u64(test_ptr);
    println!(level:info, "probe read_tables pointer: {:#x} - result: {:?}", test_ptr, probe_res);
    



    acpi::read_tables();

    clocks::init();

    acpi::init_acpi();

    drivers::init_drivers();

    drivers::pci::init();

    std::thread::sleep(core::time::Duration::from_micros(11));

    vfs::init();
    net::init();

    let res = block_task(Box::pin(vfs::mount_blkdev_partition(
        cmd_args.root_partition,
        ResolvedPath::root(),
    )));
    if let Err(e) = res {
        println!("{}", e);
        panic!("Failed to mount root partition");
    }

    proc::init();

    let mut counter = 0;
    // loop {
    //     if counter % 10 == 0 {
    //         println!(level:info, "loopng...");
    //     }
    //     std::thread::sleep(core::time::Duration::from_millis(100));
    //     counter += 1;
    //     task_runner::process_tasks();
    //     task_runner::run_repeating_tasks();
    // }

    // //start first proc
    unsafe { core::arch::asm!("int 254") };

    panic!("Returned to _start after first context switch");
}
