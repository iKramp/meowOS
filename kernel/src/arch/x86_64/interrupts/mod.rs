mod gdt;
mod pic;
use core::sync::atomic::Ordering;
use std::{println, printlnc};
#[macro_use]
pub mod handlers;
pub mod idt;
mod macros;
mod page_fault;
pub use macros::InterruptProcessorState;

use crate::arch::x86_64::interrupts::handlers::apic_eoi;

const PIC1: u16 = 0x20;
const PIC2: u16 = 0xA0; /* IO base address for slave PIC */
const PIC1_COMMAND: u16 = PIC1;
const PIC1_DATA: u16 = PIC1 + 1;
const PIC2_COMMAND: u16 = PIC2;
const PIC2_DATA: u16 = PIC2 + 1;

#[inline(always)]
pub fn enable_interrupts() {
    core::sync::atomic::fence(Ordering::Release);
    unsafe { core::arch::asm!("sti", options(nomem, nostack)) };
}

///Returns true if interrupts were enabled before this was called
#[inline(always)]
pub fn disable_interrupts() -> bool {
    let prev_rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "pop {}",
            "cli",
            out(reg) prev_rflags,
            options(nostack)
        );
    }
    core::sync::atomic::fence(Ordering::Acquire);
    (prev_rflags & (1 << 9)) != 0
}

//this is an IO EOI. Cpu EOI is handled internally
pub fn end_of_interrupt() {
    apic_eoi();
}

pub fn register_interrupt_handler(handler: InterruptHandler, interrupt_index: u64) {}

pub fn assert_interrupts_state(expected_enabled: bool) {
    let rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "pop {}",
            out(reg) rflags,
            options(nostack)
        );
    }
    let interrupts_enabled = (rflags & (1 << 9)) != 0;
    if interrupts_enabled != expected_enabled {
        panic!(
            "Interrupts state assertion failed. Expected: {}, Actual: {}",
            expected_enabled, interrupts_enabled
        );
    }
}

pub fn init_interrupts() {
    println!(level:info, "initializing PIC");
    init_pic();
    println!(level:info, "initializing GDT");
    gdt::init_boot_gdt(); //add a separate TSS for each core
    println!(level:info, "initializing IDT");
    idt::init_idt();
    printlnc!(level:info, (0, 255, 0), "interrupts initialized");
}

pub static mut APIC_TIMER_INIT: bool = false;
pub const TIMER_DESIRED_FREQUENCY: u32 = 1; //don't need much lmao
pub const PIC_TIMER_ORIGINAL_FREQ: u32 = 1_193_182;

pub fn return_interrupted(interrupt_frame: &InterruptProcessorState) -> ! {
    //make rsp at least return frame size smaller than the start of a page
    let interrupt_frame_addr: u64 = interrupt_frame as *const InterruptProcessorState as u64;
    unsafe {
        core::arch::asm!(
            "mov rsp, {0}",
            "mov r15, [rsp + 8 * 0]",
            "mov r14, [rsp + 8 * 1]",
            "mov r13, [rsp + 8 * 2]",
            "mov r12, [rsp + 8 * 3]",
            "mov r11, [rsp + 8 * 4]",
            "mov r10, [rsp + 8 * 5]",
            "mov r9,  [rsp + 8 * 6]",
            "mov r8,  [rsp + 8 * 7]",
            "mov rbp, [rsp + 8 * 8]",
            "mov rdi, [rsp + 8 * 9]",
            "mov rsi, [rsp + 8 * 10]",
            "mov rdx, [rsp + 8 * 11]",
            "mov rcx, [rsp + 8 * 12]",
            "mov rbx, [rsp + 8 * 13]",
            "mov rax, [rsp + 8 * 14]",
            //rsp + 8 * 15 is error code
            "add rsp, 8 * 16",

            "swapgs", //restore gs for user code

            "iretq",

            in(reg) interrupt_frame_addr
        );
    }
    unreachable!();
}
