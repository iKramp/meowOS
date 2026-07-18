#![allow(dead_code)]

use crate::memory::addresses::*;
use std::Vec;

#[repr(C, packed)]
pub struct Madt {
    header: super::sdt::AcpiSdtHeader,
    pub local_apic_address: u32,
    pub flags: u32, //unused, indicates if dual setup pic is present but we assume it is and mask it out
                    //anyway
}

impl Madt {
    pub fn get_madt_entries(&self) -> Vec<MadtEntryType> {
        let mut entries = Vec::new();
        let mut ptr = VirtAddr((self as *const Madt as u64) + 0x2C);
        while ptr.0 < (self as *const Madt as u64 + self.header.length as u64) {
            unsafe {
                let entry_header = get_at_addr::<MadtEntryHeader, _>(ptr);
                match entry_header.entry_type {
                    0 => {
                        entries.push(MadtEntryType::ProcessorLocalAPIC(get_at_addr::<ProcessorLocalApic, _>(ptr)));
                    }
                    1 => entries.push(MadtEntryType::IoApic(get_at_addr::<IoApic, _>(ptr))),
                    2 => entries.push(MadtEntryType::InterruptSourceOverride(get_at_addr::<
                        InterruptSourceOverride,
                        _,
                    >(ptr))),
                    3 => {
                        entries.push(MadtEntryType::NMISource(get_at_addr::<NMISource, _>(ptr)));
                    }
                    4 => entries.push(MadtEntryType::LocalApicNMI(get_at_addr::<LocalApicNMI, _>(ptr))),
                    5 => entries.push(MadtEntryType::LocalApicAddressOverride(get_at_addr::<
                        LocalApicAddressOverride,
                        _,
                    >(ptr))),
                    9 => crate::println!("x2apic not supported because idk how to parse the dsdt/ssdt"),
                    _ => entries.push(MadtEntryType::Other),
                }
                ptr.0 += entry_header.length as u64;
            }
        }
        entries
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MadtEntryType {
    ProcessorLocalAPIC(&'static ProcessorLocalApic),
    IoApic(&'static IoApic),
    InterruptSourceOverride(&'static InterruptSourceOverride),
    NMISource(&'static NMISource),
    LocalApicNMI(&'static LocalApicNMI),
    LocalApicAddressOverride(&'static LocalApicAddressOverride),

    Other,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessorLocalApicFlags(u32);

impl ProcessorLocalApicFlags {
    pub fn enabled(&self) -> bool {
        self.0 & 1 == 1
    }
    pub fn online_capable(&self) -> bool {
        self.0 & 2 == 2
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct ProcessorLocalApic {
    header: MadtEntryHeader,
    pub acpi_processor_uid: u8,
    pub apic_id: u8,
    pub flags: ProcessorLocalApicFlags,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct IoApic {
    header: MadtEntryHeader,
    pub io_apic_id: u8,
    pub reserved: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}

pub enum IntSoOverPolarity {
    Conforms,
    ActiveHigh,
    Reserved,
    ActiveLow,
}
pub enum IntSoOverTriggerMode {
    Conforms,
    EdgeTriggered,
    Reserved,
    LevelTriggered,
}
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct MpsIntiFlags {
    pub flags: u16,
}
impl MpsIntiFlags {
    pub fn polarity(&self) -> IntSoOverPolarity {
        match self.flags & 3 {
            0 => IntSoOverPolarity::Conforms,
            1 => IntSoOverPolarity::ActiveHigh,
            2 => IntSoOverPolarity::Reserved,
            3 => IntSoOverPolarity::ActiveLow,
            _ => panic!("impossible"),
        }
    }

    pub fn trigger_mode(&self) -> IntSoOverTriggerMode {
        match self.flags & 0b1100 {
            0b0000 => IntSoOverTriggerMode::Conforms,
            0b0100 => IntSoOverTriggerMode::EdgeTriggered,
            0b1000 => IntSoOverTriggerMode::Reserved,
            0b1100 => IntSoOverTriggerMode::LevelTriggered,
            _ => panic!("impossible"),
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptSourceOverride {
    header: MadtEntryHeader,
    pub bus: u8,
    pub source: u8,
    pub global_system_interrupt: u32,
    pub flags: MpsIntiFlags,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct NMISource {
    header: MadtEntryHeader,
    pub flags: MpsIntiFlags,
    pub global_system_interrupt: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct LocalApicNMI {
    header: MadtEntryHeader,
    pub acpi_processor_uid: u8,
    pub flags: MpsIntiFlags,
    pub local_apic_lint: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct LocalApicAddressOverride {
    header: MadtEntryHeader,
    pub reserved: u16,
    pub local_apic_address: u64,
}
