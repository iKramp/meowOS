#![allow(clippy::unusual_byte_groupings, static_mut_refs)]

use crate::interrupts::{disable_pic_completely, general_interrupt_handler};
use core::mem::MaybeUninit;
use std::mem_utils::PhysAddr;

use reg_map::RegMap;
use unroll::unroll_for_loops;

use crate::{
    handler,
    interrupts::{
        handlers::*,
        idt::{Entry, IDT},
    },
    memory::paging::LiminePat,
    println,
};

use super::lapic_timer::{activate_timer, setup_timer_ap};

pub static mut LAPIC_REGISTERS: MaybeUninit<LapicRegistersPtr> = MaybeUninit::uninit();

#[unroll_for_loops]
pub fn enable_apic(platform_info: &super::platform_info::PlatformInfo, processor_id: u8) {
    let bsp = processor_id == platform_info.boot_processor.processor_id;
    if bsp {
        map_lapic_registers(platform_info.apic.lapic_address);
    };

    let lapic_registers = unsafe { LAPIC_REGISTERS.assume_init_mut() };

    if bsp {
        unsafe {
            IDT.set(Entry::new(handler!(other_apic_interrupt)), 64);
            IDT.set(Entry::new(handler!(other_apic_interrupt)), 65);
            IDT.set(Entry::new(handler!(other_apic_interrupt)), 66);
            IDT.set(Entry::new(handler!(apic_error)), 67);
            IDT.set(Entry::new(handler!(other_apic_interrupt)), 68);
            IDT.set(Entry::new(handler!(other_apic_interrupt)), 69);
        }
    }

    lapic_registers
        .lvt_corrected_machine_check_interrupt()
        .bytes()
        .write(0b00000000_00000000_0_000_0_0_000_01000000_u32);

    //this constantly gives interrupts??
    lapic_registers
        .lvt_lint0()
        .bytes()
        .write(0b00000000_00000000_0_000_0_0_000_01000001_u32);
    lapic_registers
        .lvt_lint1()
        .bytes()
        .write(0b00000000_00000000_0_000_0_0_000_01000010_u32);
    lapic_registers
        .lvt_error()
        .bytes()
        .write(0b00000000_00000000_0_000_0_0_000_01000011_u32);
    lapic_registers
        .lvt_performance_monitoring_counters()
        .bytes()
        .write(0b00000000_00000000_0_000_0_0_000_01000100_u32);
    lapic_registers
        .lvt_thermal_sensor()
        .bytes()
        .write(0b00000000_00000000_0_000_0_0_000_01000101_u32);

    let mut nmi_source = 0b00000000_00000000_0_000_0_0_100_00000000_u32;
    let lapic_nmi = platform_info.get_nmi_structure(processor_id);

    if let crate::acpi::madt::IntSoOverTriggerMode::LevelTriggered = lapic_nmi.flags.trigger_mode() {
        nmi_source |= 1 << 15;
    }
    if let crate::acpi::madt::IntSoOverPolarity::ActiveLow = lapic_nmi.flags.polarity() {
        nmi_source |= 1 << 13;
    }
    if lapic_nmi.lint == 0 {
        lapic_registers.lvt_lint0().bytes().write(nmi_source);
    } else {
        lapic_registers.lvt_lint1().bytes().write(nmi_source);
    }

    //fully enable apic:
    lapic_registers
        .spurious_interrupt()
        .bytes()
        .write(0b0000000000000000000_0_00_0_1_11111111_u32);
    lapic_registers.task_priority().bytes().write(0);

    if !bsp {
        setup_timer_ap(lapic_registers);
        return;
    }
    activate_timer(lapic_registers);

    unsafe {
        IDT.set(Entry::new(handler!(apic_error)), 67);

        IDT.set(Entry::new(handler!(apic_keyboard_interrupt)), 32 + 1);
        IDT.set(Entry::new(handler!(ps2_mouse_interrupt)), 32 + 12);
        IDT.set(Entry::new(handler!(fpu_interrupt)), 32 + 13);
        IDT.set(Entry::new(handler!(primary_ata_hard_disk)), 32 + 14);
    }
    disable_pic_completely();
}

fn map_lapic_registers(lapic_address: PhysAddr) {
    unsafe {
        let lapic_virt_addr = crate::memory::PAGE_TREE_ALLOCATOR.allocate(Some(lapic_address), false);
        let apic_registers_page_entry = crate::memory::PAGE_TREE_ALLOCATOR
            .get_page_table_entry_mut(lapic_virt_addr)
            .expect("LAPIC page entry must exist after allocation");
        apic_registers_page_entry.set_pat(LiminePat::UC);
        let lapic_ref = &mut *(lapic_virt_addr.0 as *mut LapicRegisters);
        LAPIC_REGISTERS = MaybeUninit::new(LapicRegistersPtr::from_mut(lapic_ref));

        println!(
            "Mapping LAPIC registers. Phys: {:016X}, Virt: {:016X}",
            lapic_address.0, lapic_virt_addr.0
        );

        core::arch::asm!(
            "mov rax, cr3",
            "mov cr3, rax",
            out("rax") _
        ); //clear the TLB
    }
}

#[repr(C)]
#[derive(RegMap)]
pub struct LapicRegisters {
    reserved_0: LapicRegisterValueStructure,
    reserved_1: LapicRegisterValueStructure,
    lapic_id: LapicRegisterValueStructure,
    lapic_version: LapicRegisterValueStructure,
    reserved_2: LapicRegisterValueStructure,
    reserved_3: LapicRegisterValueStructure,
    reserved_4: LapicRegisterValueStructure,
    reserved_5: LapicRegisterValueStructure,
    task_priority: LapicRegisterValueStructure,
    arbitration_priority: LapicRegisterValueStructure,
    processor_priority: LapicRegisterValueStructure,
    pub end_of_interrupt: LapicRegisterValueStructure,
    remote_read: LapicRegisterValueStructure,
    logical_destination: LapicRegisterValueStructure,
    destination_format: LapicRegisterValueStructure,
    spurious_interrupt: LapicRegisterValueStructure,
    in_service: EightDWordStructure,
    trigger_mode: EightDWordStructure,
    interrupt_request: EightDWordStructure,
    pub error_status: LapicRegisterValueStructure,
    reserved_6: LapicRegisterValueStructure,
    reserved_7: LapicRegisterValueStructure,
    reserved_8: LapicRegisterValueStructure,
    reserved_9: LapicRegisterValueStructure,
    reserved_10: LapicRegisterValueStructure,
    reserved_11: LapicRegisterValueStructure,
    lvt_corrected_machine_check_interrupt: LapicRegisterValueStructure,
    interrupt_command_register_0_32: LapicRegisterValueStructure,
    interrupt_command_register_32_64: LapicRegisterValueStructure,
    pub(super) lvt_timer: LapicRegisterValueStructure,
    lvt_thermal_sensor: LapicRegisterValueStructure,
    lvt_performance_monitoring_counters: LapicRegisterValueStructure,
    lvt_lint0: LapicRegisterValueStructure,
    lvt_lint1: LapicRegisterValueStructure,
    lvt_error: LapicRegisterValueStructure,
    pub(super) initial_count: LapicRegisterValueStructure,
    pub(super) current_count: LapicRegisterValueStructure,
    reserved_12: LapicRegisterValueStructure,
    reserved_13: LapicRegisterValueStructure,
    reserved_14: LapicRegisterValueStructure,
    reserved_15: LapicRegisterValueStructure,
    pub(super) divide_configuration: LapicRegisterValueStructure,
    reserved_16: LapicRegisterValueStructure,
}

impl LapicRegistersPtr<'_> {
    pub fn send_ipi(&mut self, delivery_mode: u8, destination: u8, vector: u8) {
        self.interrupt_command_register_32_64().bytes().write((destination as u32) << 24);
        self.interrupt_command_register_0_32()
            .bytes()
            .write((vector as u32) | ((delivery_mode as u32) << 8));
    }

    pub fn send_init_ipi(&mut self, destination: u8) {
        self.send_ipi(0b101, destination, 0);
    }

    pub fn send_startup_ipi(&mut self, destination: u8, start_page: u8) {
        self.send_ipi(0b110, destination, start_page);
    }
}

#[repr(C)]
#[derive(RegMap)]
pub struct LapicRegisterValueStructure {
    pub bytes: u32,
    padding_0: u32,
    padding_1: u64,
}

#[repr(C)]
#[derive(RegMap)]
struct EightDWordStructure {
    bits_000_031: LapicRegisterValueStructure,
    bits_032_063: LapicRegisterValueStructure,
    bits_064_095: LapicRegisterValueStructure,
    bits_096_127: LapicRegisterValueStructure,
    bits_128_159: LapicRegisterValueStructure,
    bits_160_191: LapicRegisterValueStructure,
    bits_192_223: LapicRegisterValueStructure,
    bits_224_255: LapicRegisterValueStructure,
}

#[allow(clippy::identity_op)]
impl EightDWordStructure {
    pub fn write(&mut self, low: u128, high: u128) {
        self.bits_000_031.bytes = ((low >> 00) & 0xFFFFFFFF) as u32;
        self.bits_032_063.bytes = ((low >> 32) & 0xFFFFFFFF) as u32;
        self.bits_064_095.bytes = ((low >> 64) & 0xFFFFFFFF) as u32;
        self.bits_096_127.bytes = ((low >> 96) & 0xFFFFFFFF) as u32;
        self.bits_128_159.bytes = ((high >> 00) & 0xFFFFFFFF) as u32;
        self.bits_160_191.bytes = ((high >> 32) & 0xFFFFFFFF) as u32;
        self.bits_192_223.bytes = ((high >> 64) & 0xFFFFFFFF) as u32;
        self.bits_224_255.bytes = ((high >> 96) & 0xFFFFFFFF) as u32;
    }

    #[allow(clippy::identity_op)]
    pub fn read(&self) -> (u128, u128) {
        (
            ((self.bits_000_031.bytes as u128) << 00)
                | ((self.bits_032_063.bytes as u128) << 32)
                | ((self.bits_064_095.bytes as u128) << 64)
                | ((self.bits_096_127.bytes as u128) << 96),
            ((self.bits_128_159.bytes as u128) << 00)
                | ((self.bits_160_191.bytes as u128) << 32)
                | ((self.bits_192_223.bytes as u128) << 64)
                | ((self.bits_224_255.bytes as u128) << 96),
        )
    }
}

#[repr(C)]
struct TwoDWordStructure {
    pub bits_000_031: LapicRegisterValueStructure,
    pub bits_032_063: LapicRegisterValueStructure,
}

impl TwoDWordStructure {
    pub fn write(&mut self, val: u64) {
        self.bits_000_031.bytes = (val & 0xFFFFFFFF) as u32;
        self.bits_000_031.bytes = (val & 0xFFFFFFFF) as u32;
    }

    pub fn read(&self) -> u64 {
        self.bits_000_031.bytes as u64 | ((self.bits_032_063.bytes as u64) << 32)
    }
}
