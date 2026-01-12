use core::fmt::Debug;
use std::{error::ErrorCode, println};

use crate::interrupts::{InterruptProcessorState, handlers::apic_eoi};

mod bar;
mod capabilities;
mod device_class;
mod device_codes;
mod express;
mod legacy;
mod port_access;

pub use bar::*;
pub use express::express_device::PcieDevice;
pub use legacy::legacy_device::LegacyPciDevice;
pub(super) use legacy::add_legacy_pci_driver;
pub(super) use express::add_pcie_driver;
pub(super) use device_class::*;
pub(super) use device_codes::PciDeviceNumericId;

pub(super) const PCI_CAP_POWER_MANAGEMENT_ID: u8 = 0x1;
pub(super) const PCI_CAP_PCIE_ID: u8 = 0x10;

#[derive(Debug, Clone)]
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

pub static mut PCI_DEVICE_INTERRUPTS: [(u8, u8, u8); 256] = [(255, 255, 255); 256];

//pci interrupt handler
pub extern "C" fn pci_interrupt(_proc_data: &mut InterruptProcessorState) {
    println!("PCI interrupt. HOW THE HELL DO I KNOW WHAT DEVICE THIS IS FOR?");
    apic_eoi();
    panic!("PCI interrupt received");
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
