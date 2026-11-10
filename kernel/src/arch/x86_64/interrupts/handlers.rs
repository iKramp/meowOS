use crate::arch::interrupts::apic::LAPIC_REGISTERS;
use crate::arch::interrupts::gdt::GlobalDescriptorTable;
use crate::interrupts::InterruptReturnType;
use crate::memory::addresses::*;
use crate::proc::ProcessData;
use crate::{acpi::cpu_locals::CpuLocals, drivers::ps2, proc::context_switch, utils::byte_to_port};
#[allow(unused_imports)] //they are used in macros
use core::arch::asm;
use std::sync::arc::Arc;
use std::{println, printlnc};

use super::macros::InterruptProcessorState;

pub extern "C" fn invalid_opcode(proc_data: &mut InterruptProcessorState) {
    printlnc!(level:error,
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

pub extern "C" fn breakpoint(
    cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    printlnc!(level:warn,
        (0, 255, 255),
        "Breakpoint reached at {:#X}:{:#X}",
        cpu_data.interrupt_frame.cs,
        cpu_data.interrupt_frame.rip
    );
    apic_eoi();
    legacy_eoi();
    InterruptReturnType::Normal
}

//gpf
pub extern "C" fn general_protection_fault(
    cpu_data: &mut InterruptProcessorState,
    _proc_state: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    printlnc!(level:error,(0, 0, 255), "EXCEPTION: GPF. err code: {:#X?}", cpu_data.err_code);
    printlnc!(level:error,(0, 0, 255), "EXCEPTION: GPF. proc_data: {:#X?}", cpu_data);
    //print GDT
    let cpu_locals = CpuLocals::get();
    let gdt_ptr = cpu_locals.arch_specific.gdt_ptr;
    let gdt = unsafe { get_at_addr::<GlobalDescriptorTable, _>(VirtAddr(gdt_ptr.base)) };
    println!(level:error,"gdt: {:#x?}", gdt);
    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

pub extern "C" fn other_legacy_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_state: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    printlnc!((0, 0, 255), "interrupt: OTHER LEGACY INTERRUPT");
    panic!();
    // legacy_eoi();
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

pub extern "C" fn other_apic_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn apic_timer_tick(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn legacy_timer_tick(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    // just to resume from hlt
    legacy_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn apic_error(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    let lapic_registers = unsafe { LAPIC_REGISTERS.assume_init_mut() };
    lapic_registers.error_status().bytes().write(0); //activate it to load the real value
    let _error_val = &lapic_registers.error_status().bytes().read();
    //do error shit
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn spurious_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn legacy_keyboard_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    ps2::handle_ps2_interrupt(1);
    legacy_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn apic_keyboard_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    ps2::handle_ps2_interrupt(1);
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn legacy_mouse_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    ps2::handle_ps2_interrupt(2);
    legacy_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn apic_mouse_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    ps2::handle_ps2_interrupt(2);
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn fpu_interrupt(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn primary_ata_hard_disk(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    apic_eoi();
    InterruptReturnType::Normal
}

pub extern "C" fn first_context_switch(
    _cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    let mut locals = CpuLocals::get_mut();
    locals.int_depth = 1; //this one context switches back into 0
    locals.proc_initialized = true;
    drop(locals);
    context_switch();
    InterruptReturnType::Normal
}

pub extern "C" fn inter_processor_interrupt(
    cpu_data: &mut InterruptProcessorState,
    _proc_data: Option<&Arc<ProcessData>>,
) -> InterruptReturnType {
    // This is a placeholder for inter-processor interrupts
    // Currently, it just acknowledges the interrupt
    apic_eoi();
    printlnc!((0, 255, 0), "Inter-processor interrupt received: {:#X?}", cpu_data);
    InterruptReturnType::Normal
}
