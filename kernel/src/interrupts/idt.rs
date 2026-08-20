use crate::handler;
use crate::interrupts::macros::InterruptProcessorState;
use crate::interrupts::page_fault::page_fault;

use super::gdt::{DEBUG_IST, DOUBLE_FAULT_IST, MACHINE_CHECK_IST, NMI_IST};
use super::handlers::*;
use core::arch::asm;
use std::printlnc;

macro_rules! never_exit_interrupt_message {
    ($message:expr, $func_name:ident) => {
        extern "C" fn $func_name(proc_data: &mut InterruptProcessorState) -> ! {
            printlnc!(level:error, (0, 0, 255), "{} exception", $message);
            printlnc!(level:error,
                (0, 0, 255),
                "segmetn:instruction: {:x}:{:x}",
                proc_data.interrupt_frame.cs,
                proc_data.interrupt_frame.rip
            );
            loop {}
        }
    };
}

never_exit_interrupt_message!("divide_by_zero", divide_by_zero_handler);
never_exit_interrupt_message!("debug", debug_handler);
never_exit_interrupt_message!("non maskable interrupt", nmi_handler);
never_exit_interrupt_message!("overflow", overflow_handler);
never_exit_interrupt_message!("bound range exceeded", bound_handler);
never_exit_interrupt_message!("invalid opcode", invalid_opcode_handler);
never_exit_interrupt_message!("device not available", device_not_available_handler);
never_exit_interrupt_message!("double fault", double_fault_handler);
never_exit_interrupt_message!("coprocessor segment overrun", coprocessor_segment_overrun_handler);
never_exit_interrupt_message!("invalid TSS", invalid_tss_handler);
never_exit_interrupt_message!("segment not present", segment_not_present_handler);
never_exit_interrupt_message!("stack segment fault", ss_fault_handler);
never_exit_interrupt_message!("reserved", reserved_handler);
never_exit_interrupt_message!("fpu error", fpu_error_handler);
never_exit_interrupt_message!("alignment check", alignment_check_handler);
never_exit_interrupt_message!("machine check", machine_check_handler);
never_exit_interrupt_message!("simd fp", simd_fp_handler);
never_exit_interrupt_message!("virtualization", virtualization_handler);
never_exit_interrupt_message!("control", control_handler);

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TablePointer {
    pub limit: u16,
    pub base: u64,
}

#[used]
pub static mut IDT_POINTER: TablePointer = TablePointer { limit: 0, base: 0 };

const IDT_SIZE: usize = 256;

#[repr(align(4096))]
pub struct Idt {
    entry_table: [Entry; IDT_SIZE],
}

pub static mut IDT: Idt = Idt::new();

impl Idt {
    pub const fn new() -> Self {
        Idt {
            entry_table: [Entry::missing(); IDT_SIZE],
        }
    }

    pub fn set(&mut self, entry: Entry, index: usize) {
        self.entry_table[index] = entry;
    }

    pub fn load(&'static self) {
        unsafe {
            IDT_POINTER = TablePointer {
                base: self as *const _ as u64,
                limit: (core::mem::size_of::<Self>() - 1) as u16,
            };
            asm!("lidt [{}]", "sti", in(reg) core::ptr::addr_of!(IDT_POINTER));
        }
    }

    pub fn set_entries(&mut self) {
        self.set(Entry::new(handler!(divide_by_zero_handler)), 0);
        self.set(Entry::ist_index(DEBUG_IST, handler!(debug_handler, slow_swap)), 1);
        self.set(Entry::ist_index(NMI_IST, handler!(nmi_handler, slow_swap)), 2);
        self.set(Entry::new(handler!(breakpoint)), 3);
        self.set(Entry::new(handler!(overflow_handler)), 4);
        self.set(Entry::new(handler!(bound_handler)), 5);
        self.set(Entry::new(handler!(invalid_opcode_handler)), 6);
        self.set(Entry::new(handler!(device_not_available_handler)), 7);
        self.set(
            Entry::ist_index(DOUBLE_FAULT_IST, handler!(double_fault_handler, slow_swap, has_code)),
            8,
        );
        self.set(Entry::new(handler!(coprocessor_segment_overrun_handler)), 9);
        self.set(Entry::new(handler!(invalid_tss_handler, has_code)), 10);
        self.set(Entry::new(handler!(segment_not_present_handler, has_code)), 11);
        self.set(Entry::new(handler!(ss_fault_handler, has_code)), 12);
        self.set(Entry::new(handler!(general_protection_fault, has_code)), 13);
        self.set(Entry::new(handler!(page_fault, has_code)), 14);
        self.set(Entry::new(handler!(reserved_handler)), 15);
        self.set(Entry::new(handler!(fpu_error_handler)), 16);
        self.set(Entry::new(handler!(alignment_check_handler, has_code)), 17);
        self.set(
            Entry::ist_index(MACHINE_CHECK_IST, handler!(machine_check_handler, slow_swap)),
            18,
        );
        self.set(Entry::new(handler!(simd_fp_handler)), 19);
        self.set(Entry::new(handler!(virtualization_handler)), 20);
        self.set(Entry::new(handler!(control_handler, has_code)), 21);
        self.set(Entry::new(handler!(reserved_handler)), 22);
        self.set(Entry::new(handler!(reserved_handler)), 23);
        self.set(Entry::new(handler!(reserved_handler)), 24);
        self.set(Entry::new(handler!(reserved_handler)), 25);
        self.set(Entry::new(handler!(reserved_handler)), 26);
        self.set(Entry::new(handler!(reserved_handler)), 27);
        self.set(Entry::new(handler!(reserved_handler)), 28);
        self.set(Entry::new(handler!(reserved_handler)), 29);
        self.set(Entry::new(handler!(reserved_handler)), 30);
        self.set(Entry::new(handler!(reserved_handler)), 31);

        for i in 32..256 {
            self.set(Entry::new(handler!(other_legacy_interrupt)), i);
        }

        self.set(Entry::new(handler!(legacy_timer_tick)), 32);
        self.set(Entry::new(handler!(legacy_keyboard_interrupt)), 33);

        self.set(Entry::new(handler!(apic_timer_tick)), 100);
        self.set(Entry::new(handler!(inter_processor_interrupt)), 101);
        self.set(Entry::new(handler!(first_context_switch)), 254);
        self.set(Entry::new(handler!(spurious_interrupt)), 255);

        //entries set by other files:
        //32 - 64 - ioapic with 32 offset
        //32 - pit timer
        //33 - apic keyboard
        //32 + 12 (44) - ps2 mouse
        //32 + 13 (45) - fpu
        //32 + 14 (46) - ata????
        //67 - apic error
        //251 - selected timekeeping timer (100 is free to use after apic init)
        //252 - lapic timer
        //253 - inter processor interrupt
        //254 - first context switch
        //255 - spurious interrupt
        //128..160 - PCI interrupts
    }
}

pub fn init_idt() {
    unsafe {
        IDT.set_entries();
        IDT.load();
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: u16,
    options: u16,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl Entry {
    pub fn new_custom(gdt_selector: u16, handler: extern "C" fn() -> !, options: u16) -> Self {
        let pointer = handler as usize;
        Self {
            gdt_selector,
            pointer_low: (pointer & 0xFFFF) as u16,
            pointer_middle: ((pointer & 0xFFFF0000) >> 16) as u16,
            pointer_high: ((pointer & 0xFFFFFFFF00000000) >> 32) as u32,
            options,
            reserved: 0,
        }
    }

    pub fn new(handler: extern "C" fn() -> !) -> Self {
        Self::new_custom(0x8, handler, construct_entry_options(0, false, 0, true))
    }

    pub fn ist_index(ist_index: u16, handler: extern "C" fn() -> !) -> Self {
        Self::new_custom(0x8, handler, construct_entry_options(ist_index, false, 0, true))
    }

    const fn missing() -> Self {
        Self {
            gdt_selector: 0,
            pointer_high: 0,
            pointer_middle: 0,
            pointer_low: 0,
            options: 0,
            reserved: 0,
        }
    }
}

//privilige level is always 0 (kernel)
//always interrupt gate, which clears IF
//type bits 11 10 9 8
//
const fn construct_entry_options(
    interrupt_stack_table_index: u16,
    interrupt_gate: bool,
    descriptor_privilege_level: u16,
    present: bool,
) -> u16 {
    assert!(interrupt_stack_table_index < 8);
    assert!(descriptor_privilege_level < 4);
    let mut num: u16 = 0b0000_1110_0000_0000 | interrupt_stack_table_index | (descriptor_privilege_level << 13);
    if present {
        num |= 1 << 15;
    }
    if interrupt_gate {
        num |= 1 << 8;
    }
    num
}
