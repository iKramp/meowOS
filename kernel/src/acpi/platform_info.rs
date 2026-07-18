use crate::memory::addresses::PhysAddr;

use super::madt::{MadtEntryType, MpsIntiFlags};
use std::println;

static mut BOOT_FOUND: bool = false;

#[derive(Default, Debug)]
pub struct PlatformInfo {
    pub apic: Apic,
    pub boot_processor: Processor,
    pub application_processors: std::Vec<Processor>,
    pub max_apic_id: u8,
}

impl PlatformInfo {
    pub fn new(madt_entries: &std::Vec<super::madt::MadtEntryType>, apic_address: PhysAddr) -> Self {
        let mut info = Self::default();
        info.apic.lapic_address = apic_address;
        for entry in madt_entries {
            match entry {
                MadtEntryType::ProcessorLocalAPIC(data) => {
                    let processor = Processor {
                        processor_id: data.acpi_processor_uid,
                        apic_id: data.apic_id,
                        flags: data.flags,
                    };
                    info.max_apic_id = info.max_apic_id.max(processor.apic_id);
                    if !processor.flags.enabled() {
                        if processor.flags.online_capable() {
                            println!("processor {} is not enabled but online capable", processor.apic_id);
                        } else {
                            println!("processor {} is not enabled and not online capable", processor.apic_id);
                        }
                        continue;
                    }
                    if unsafe { BOOT_FOUND } {
                        info.application_processors.push(processor)
                    } else {
                        unsafe {
                            BOOT_FOUND = true;
                        }
                        info.boot_processor = processor;
                    }
                }
                MadtEntryType::IoApic(data) => {
                    info.apic.io_apics.push(IOApic {
                        id: data.io_apic_id,
                        address: data.io_apic_address,
                        global_system_interrupt_base: data.global_system_interrupt_base,
                    });
                }
                MadtEntryType::LocalApicAddressOverride(data) => {
                    info.apic.lapic_address = PhysAddr(data.local_apic_address);
                }
                MadtEntryType::NMISource(_data) => {
                    todo!("nmi sources are not yet implemented");
                    //info.apic.nmi_source.push() //osdev wiki and uefi.org have conflicting
                    //definitions
                }
                MadtEntryType::LocalApicNMI(data) => {
                    let target = if data.acpi_processor_uid == 0xff {
                        NMILineTarget::All
                    } else {
                        NMILineTarget::Id(data.acpi_processor_uid)
                    };
                    info.apic.local_apic_nmi_lines.push(NMILine {
                        target,
                        flags: data.flags,
                        lint: data.local_apic_lint,
                    });
                }
                MadtEntryType::InterruptSourceOverride(data) => {
                    if data.bus != 0 {
                        panic!("bus isn't 0 which is not supported");
                    }
                    info.apic.interrupt_source_overrides.push(InterruptSourceOverride {
                        source: data.source,
                        global_system_interrupt: data.global_system_interrupt,
                        flags: data.flags,
                    });
                }
                _ => {}
            }
        }
        //println!("apic address is {:#x?}", info.apic.lapic_address);
        info
    }

    pub fn get_nmi_structure(&self, processor_id: u8) -> NMILine {
        for line in &self.apic.local_apic_nmi_lines {
            if line.target == NMILineTarget::All || line.target == NMILineTarget::Id(processor_id) {
                return *line;
            }
        }
        NMILine {
            target: NMILineTarget::Id(processor_id),
            flags: MpsIntiFlags { flags: 0 },
            lint: 1,
        }
    }
}

#[derive(Default, Debug)]
pub struct Apic {
    pub lapic_address: PhysAddr,
    pub io_apics: std::Vec<IOApic>,
    pub local_apic_nmi_lines: std::Vec<NMILine>,
    pub interrupt_source_overrides: std::Vec<InterruptSourceOverride>,
    pub nmi_source: std::Vec<()>,
}

#[derive(Debug, Default)]
pub struct Processor {
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: super::madt::ProcessorLocalApicFlags,
}

#[derive(Debug)]
pub struct IOApic {
    pub id: u8,
    pub address: u32,
    pub global_system_interrupt_base: u32,
}

#[derive(Debug)]
pub struct InterruptSourceOverride {
    pub source: u8,
    pub global_system_interrupt: u32,
    pub flags: super::madt::MpsIntiFlags,
}

pub struct LocalAPICNMI {
    processor_id: u8,
    flags: super::madt::MpsIntiFlags,
    local_apic_lint: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NMILineTarget {
    All,
    Id(u8),
}
#[derive(Debug, Clone, Copy)]
pub struct NMILine {
    pub target: NMILineTarget,
    pub flags: super::madt::MpsIntiFlags,
    pub lint: u8,
}
