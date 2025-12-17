use crate::{
    acpi::{LAPIC_REGISTERS, cpu_locals::CpuLocals},
    interrupts::gdt::GlobalDescriptorTable,
    memory::paging::PageTree,
    proc::context_switch,
    utils::{byte_from_port, byte_to_port},
};
#[allow(unused_imports)] //they are used in macros
use core::arch::asm;
use std::{
    mem_utils::{VirtAddr, get_at_virtual_addr},
    println, printlnc,
};

use super::macros::InterruptProcessorState;

pub extern "C" fn invalid_opcode(proc_data: &mut InterruptProcessorState) {
    printlnc!(
        (0, 0, 255),
        "EXCEPTION: INVALID OPCODE at {:#X}:{:#X}",
        proc_data.interrupt_frame.cs,
        proc_data.interrupt_frame.rip
    );
    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

pub extern "C" fn breakpoint(proc_data: &mut InterruptProcessorState) {
    printlnc!(
        (0, 255, 255),
        "Breakpoint reached at {:#X}:{:#X}",
        proc_data.interrupt_frame.cs,
        proc_data.interrupt_frame.rip
    );
    apic_eoi();
    legacy_eoi();
}

#[derive(Debug)]
#[allow(dead_code)] //not actually dead, is used in println
struct PageFaultErrorCode {
    protection_violation: bool,
    caused_by_write: bool,
    user_mode: bool,
    malformed_table: bool,
    instruction_fetch: bool,
}

impl From<u64> for PageFaultErrorCode {
    fn from(value: u64) -> Self {
        Self {
            protection_violation: value & (1 << 0) != 0,
            caused_by_write: value & (1 << 1) != 0,
            user_mode: value & (1 << 2) != 0,
            malformed_table: value & (1 << 3) != 0,
            instruction_fetch: value & (1 << 4) != 0,
        }
    }
}

pub extern "C" fn page_fault(proc_data: &mut InterruptProcessorState) {
    println!("{}", proc_data as *const InterruptProcessorState as usize);
    printlnc!(
        (0, 0, 255),
        "EXCEPTION: PAGE FAULT. error code: {:#X?}\nproc state: {:#X?}",
        PageFaultErrorCode::from(proc_data.err_code),
        proc_data
    );
    let mut page_tree = PageTree::new(PageTree::get_level4_addr());
    page_tree
        .get_page_table_entry_mut(VirtAddr(proc_data.interrupt_frame.rip & 0xFFFF_FFFF_FFFF_F000))
        .map(|entry| {
            println!(
                "Page fault at {:#X?} with entry: {:#X?}",
                proc_data.interrupt_frame.rip, entry
            );
        })
        .unwrap_or_else(|| {
            println!("Page fault at {:#X?} with no entry", proc_data.interrupt_frame.rip);
        });
    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

//gpf
pub extern "C" fn general_protection_fault(proc_data: &mut InterruptProcessorState) {
    printlnc!((0, 0, 255), "EXCEPTION: GPF. err code: {:#X?}", proc_data.err_code);
    printlnc!((0, 0, 255), "EXCEPTION: GPF. proc_data: {:#X?}", proc_data);
    //print GDT
    let cpu_locals = CpuLocals::get();
    let gdt_ptr = cpu_locals.gdt_ptr;
    let gdt = unsafe { get_at_virtual_addr::<GlobalDescriptorTable>(VirtAddr(gdt_ptr.base)) };
    println!("gdt: {:#x?}", gdt);
    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

pub extern "C" fn other_legacy_interrupt(_proc_data: &mut InterruptProcessorState) {
    printlnc!((0, 0, 255), "interrupt: OTHER LEGACY INTERRUPT");
    legacy_eoi();
}

#[inline]
pub fn apic_eoi() {
    let lapic_registers = unsafe { LAPIC_REGISTERS.assume_init_mut() };
    lapic_registers.end_of_interrupt().bytes().write(0);
}

#[inline]
fn legacy_eoi() {
    byte_to_port(0x20, 0x20);
    byte_to_port(0xA0, 0x20);
}

pub extern "C" fn other_apic_interrupt(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
}

pub extern "C" fn apic_timer_tick(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
}

pub extern "C" fn legacy_timer_tick(_proc_data: &mut InterruptProcessorState) {
    // just to resume from hlt
    legacy_eoi();
}

pub extern "C" fn apic_error(_proc_data: &mut InterruptProcessorState) {
    let lapic_registers = unsafe { LAPIC_REGISTERS.assume_init_mut() };
    lapic_registers.error_status().bytes().write(0); //activate it to load the real value
    let _error_val = &lapic_registers.error_status().bytes().read();
    //do error shit
    apic_eoi();
}

pub extern "C" fn spurious_interrupt(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
}

pub extern "C" fn legacy_keyboard_interrupt(_proc_data: &mut InterruptProcessorState) {
    let code = byte_from_port(0x60);
    //println!("{code}");
    crate::keyboard::handle_key(code);
    legacy_eoi();
}

pub extern "C" fn apic_keyboard_interrupt(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
    let code = byte_from_port(0x60);
    crate::keyboard::handle_key(code);
    //println!("{code}");
}

pub extern "C" fn ps2_mouse_interrupt(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
}

pub extern "C" fn fpu_interrupt(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
}

pub extern "C" fn primary_ata_hard_disk(_proc_data: &mut InterruptProcessorState) {
    apic_eoi();
}

pub extern "C" fn first_context_switch(_proc_data: &mut InterruptProcessorState) {
    let mut locals = CpuLocals::get_mut();
    locals.int_depth = 1; //this one context switches back into 0
    locals.proc_initialized = true;
    drop(locals);
    context_switch();
}

pub extern "C" fn inter_processor_interrupt(proc_data: &mut InterruptProcessorState) {
    // This is a placeholder for inter-processor interrupts
    // Currently, it just acknowledges the interrupt
    apic_eoi();
    printlnc!((0, 255, 0), "Inter-processor interrupt received: {:#X?}", proc_data);
}
