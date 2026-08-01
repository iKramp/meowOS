#![no_std]
#![no_main]
#![feature(stmt_expr_attributes)]
#![feature(string_remove_matches)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]
#![feature(str_from_raw_parts)]
#![feature(rustc_attrs)]
#![feature(map_try_insert)]
#![allow(incomplete_features)]
#![feature(async_drop)]
#![allow(internal_features)]
#![allow(clippy::fn_to_numeric_cast)]

extern crate static_cond;

use core::ffi;
use std::{println, printlnc};

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
mod net;
mod parsers;
mod printer;
mod proc;
mod rand;
mod shell;
mod task_runner;
mod tty;
mod utils;
mod vfs;
mod vga;
use limine::LIMINE_BOOTLOADER_REQUESTS;
use vfs::ResolvedPath;

use crate::{memory::addresses::VirtAddr, task_runner::block_task};

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

    memory::init_memory(VirtAddr(stack_pointer as u64));

    acpi::cpu_locals::init_dummy_cpu_locals();

    interrupts::init_interrupts();

    let cmd_args = cmd_args::CmdArgs::new(str.to_str().expect("Invalid UTF-8 in cmdline"));
    println!(level:info, "cmd_args: {:?}", cmd_args);

    let test_ptr = acpi::read_tables as *const () as u64;
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

    let future = vfs::mount_blkdev_partition(cmd_args.root_partition, ResolvedPath::root());
    let ffi_future = std::ffi_future::future::into_ffi_future(future);
    let res = block_task(ffi_future);
    if let Err(e) = res {
        println!(level:error, "{}", e);
        panic!("Failed to mount root partition");
    }

    proc::init();
    shell::init();

    // //start first proc
    unsafe { core::arch::asm!("int 254") };

    panic!("Returned to _start after first context switch");
}
