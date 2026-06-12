mod gdt;
use core::sync::atomic::Ordering;
pub use gdt::{STATIC_GDT_PTR, create_new_gdt, load_gdt};
use std::{println, printlnc};
#[macro_use]
pub mod handlers;
pub mod idt;
mod macros;
mod page_fault;
use crate::utils::byte_to_port;
pub use macros::InterruptProcessorState;
pub use macros::general_interrupt_handler;

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

pub fn init_interrupts() {
    println!(level:info, "initializing PIC");
    init_pic();
    println!(level:info, "initializing GDT");
    gdt::init_boot_gdt(); //add a separate TSS for each core
    println!(level:info, "initializing IDT");
    idt::init_idt();
    printlnc!(level:info, (0, 255, 0), "interrupts initialized");
}

pub fn init_pic() {
    byte_to_port(PIC1_COMMAND, 0x11);
    byte_to_port(PIC2_COMMAND, 0x11);

    byte_to_port(PIC1_DATA, 0x20);
    byte_to_port(PIC2_DATA, 0x28);

    byte_to_port(PIC1_DATA, 0x04);
    byte_to_port(PIC2_DATA, 0x02);

    byte_to_port(PIC1_DATA, 0x01);
    byte_to_port(PIC2_DATA, 0x01);

    disable_timer();

    byte_to_port(PIC1_DATA, 0xFE); //only allow timer
    byte_to_port(PIC2_DATA, 0xFE);
}

pub fn disable_pic_keep_timer() {
    byte_to_port(PIC1_DATA, 0xFE); //mask interrupts, keep timer
    byte_to_port(PIC2_DATA, 0xFE);

    byte_to_port(PIC1_DATA - 1, 0x20); //trigger EOI
    byte_to_port(PIC2_DATA - 1, 0x20);
}

pub fn disable_pic_completely() {
    byte_to_port(PIC1_DATA, 0xFF); //mask interrupts
    byte_to_port(PIC2_DATA, 0xFF);

    byte_to_port(PIC1_DATA - 1, 0x20); //trigger EOI
    byte_to_port(PIC2_DATA - 1, 0x20);

    disable_timer();

    disconnect_imcr();
}

fn disconnect_imcr() {
    const IMCR: u16 = 0x22;

    byte_to_port(IMCR, 0x70);
    byte_to_port(IMCR + 1, 0x01);
}

pub static mut APIC_TIMER_INIT: bool = false;
pub const TIMER_DESIRED_FREQUENCY: u32 = 1; //don't need much lmao
pub const PIC_TIMER_ORIGINAL_FREQ: u32 = 1_193_182;

fn disable_timer() {
    #[allow(clippy::unusual_byte_groupings)]
    byte_to_port(0x43, 0b00_11_000_0);
}

///Max 50 miliseconds
pub fn set_pit_timeout(timeout_nanoseconds: u32) {
    let divisor = PIC_TIMER_ORIGINAL_FREQ as u64 * timeout_nanoseconds as u64 / 1_000_000_000;
    let divisor_low = (divisor & 0xFF) as u8;
    let divisor_high = ((divisor >> 8) & 0xFF) as u8;

    //one-shot mode
    #[allow(clippy::unusual_byte_groupings)]
    byte_to_port(0x43, 0b00_11_000_0); // set mode to one-shot
    byte_to_port(0x40, divisor_low); // set low byte
    byte_to_port(0x40, divisor_high); // set high byte
}

pub fn trigger_pit_eoi() {
    // Trigger EOI for PIT
    byte_to_port(PIC1_DATA - 1, 0x20);
    byte_to_port(PIC2_DATA - 1, 0x20);
}
