use std::{error::ErrorCode, println, vec::Vec};

use crate::drivers::pci::{
    LegacyPciDevice, PCI_CAP_PCIE_ID, PciDeviceLocation, device_class::PciClass, device_codes::PciDeviceNumericId, port_access,
};

pub(super) mod config_space;
pub(super) mod legacy_device;

type LegacyPciDevDriverInitFn = ((PciClass, PciDeviceNumericId), fn(LegacyPciDevice));

pub trait LegacyPciDriver: Send + Sync {
    fn init(&mut self, dev: &LegacyPciDevice) -> Result<(), ErrorCode>;
    fn deinit(&mut self, dev: &LegacyPciDevice);
    fn service_interrupt(&mut self, dev: &LegacyPciDevice);
    /// Called after the device is removed from the system
    /// Either forcibly, or deinit was called earlier
    fn remove_device(&mut self);
}

pub fn get_devices() -> Vec<LegacyPciDevice> {
    let devices = scan_pci_bus();

    devices.iter().filter_map(|dev_location| {
        println!(
            "pci::enumerate_devices: Found device at {:02x}:{:02x}.{:x}",
            dev_location.bus, dev_location.device, dev_location.function
        );

        let device = LegacyPciDevice::new(*dev_location);
        if device.common.capabilities.iter().any(|cap| cap.id == PCI_CAP_PCIE_ID) {
            println!(
                "Found PCIe device at {:02x}:{:02x}.{:x}",
                device.common.device.bus, device.common.device.device, device.common.device.function
            );
            println!("It will be initialized later from the MCFG table");
            None
        } else {
            // init_pci_device(&device);
            Some(device)
        }
    }).collect()
}

fn scan_pci_bus() -> Vec<PciDeviceLocation> {
    let mut devices = Vec::new();
    for bus in 0..=255 {
        for device in 0..32 {
            for function in 0..8 {
                //all IDs are valid
                let device = unsafe { PciDeviceLocation::new(None, bus, device, function).unwrap_unchecked() };
                let first_dword = port_access::get_dword(0, &device);
                let vendor_id = first_dword as u16;
                if vendor_id == 0xFFFF {
                    if function == 0 {
                        break;
                    } else {
                        continue;
                    }
                }
                devices.push(device);
            }
        }
    }
    devices
}
