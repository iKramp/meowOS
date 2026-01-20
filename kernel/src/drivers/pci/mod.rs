use crate::interrupts::general_interrupt_handler;
use core::fmt::Debug;
use std::{error::ErrorCode, println};

use crate::{
    handler,
    interrupts::{InterruptProcessorState, handlers::apic_eoi},
};

mod bar;
mod capabilities;
mod device_class;
mod device_codes;
mod express;
mod legacy;
mod port_access;

pub use bar::*;
pub(super) use device_class::*;
pub(super) use device_codes::PciDeviceNumericId;
pub(super) use express::add_pcie_driver;
pub use express::express_device::PcieDevice;
pub(super) use legacy::add_legacy_pci_driver;
pub use legacy::legacy_device::LegacyPciDevice;

pub(super) const PCI_CAP_POWER_MANAGEMENT_ID: u8 = 0x1;
pub(super) const PCI_CAP_PCIE_ID: u8 = 0x10;

#[derive(Debug, Clone, Copy)]
struct PciDevice {
    group: u16,
    bus: u8,
    device: u8,
    function: u8,
}

enum FullPciDevType<'a> {
    Legacy(&'a LegacyPciDevice),
    Express(&'a PcieDevice),
}

impl PciDevice {
    pub fn new(group: Option<u16>, bus: u8, device: u8, function: u8) -> Result<Self, ErrorCode> {
        if device > 31 || function > 7 {
            return Err(ErrorCode::InvalidArgument);
        }
        Ok(Self {
            group: group.unwrap_or(0),
            bus,
            device,
            function,
        })
    }
}

pub fn enumerate_devices() {
    legacy::configure_devices();
    express::configure_devices();
}

pub static mut PCI_DEVICE_INTERRUPTS: [PciDevice; 256] = [PciDevice {
    group: 0,
    bus: 0,
    device: 0,
    function: 0,
}; 256];

//pci interrupt handler
pub extern "C" fn pci_interrupt(_proc_data: &mut InterruptProcessorState) {
    println!("PCI interrupt. HOW THE HELL DO I KNOW WHAT DEVICE THIS IS FOR?");
    apic_eoi();
    panic!("PCI interrupt received");
}

fn set_interrupt_handler(index: u8, device: PciDevice) {
    let entry = crate::interrupts::idt::Entry::new(handler!(pci_interrupt));
    unsafe {
        crate::interrupts::idt::IDT.set(entry, index as usize);
    }
    unsafe {
        PCI_DEVICE_INTERRUPTS[index as usize] = device;
    }
}

pub trait PCIDriver: Debug {
    fn class(&self) -> device_class::PciClass;
    fn vendor_id(&self) -> Option<u16> {
        None
    }
    fn device_id(&self) -> Option<u16> {
        None
    }
}
