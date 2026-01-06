use std::{boxed::Box, print, println, printlnc, vec::Vec};

use crate::{drivers::{ahci::disk::AhciController, pci::{FullPciDevType, LegacyPciDevice, PCI_CAP_PCIE_ID, PciDevice, capabilities, device_class::{self, MassStorageController}, device_codes::DeviceIdentification, port_access}}, task_runner::block_task};

pub(super) mod legacy_device;
pub(super) mod config_space;

pub fn configure_devices() {
    let devices = scan_pci_bus();

    for device in devices {
        println!("pci::enumerate_devices: Found device at {:02x}:{:02x}.{:x}", device.bus, device.device, device.function);

        let device = LegacyPciDevice::new(device);
        if device.capabilities.iter().any(|cap| cap.id == PCI_CAP_PCIE_ID) {
            println!(
                "Found PCIe device at {:02x}:{:02x}.{:x}",
                device.device.bus, device.device.device, device.device.function
            );
            println!("It will be initialized later from the MCFG table");
        } else {
            init_pci_device(device);
        }
    }

}

fn init_pci_device(dev: LegacyPciDevice) {
    let class = config_space::get_class(&dev.device);
    let header_type = config_space::get_header_type(&dev.device) & 0x7F;
    if header_type != 0 {
        println!("skipping device {:#X?} because i don't configure bridges yet", class);
        return;
    }
    println!("@DBG configuring pcie device (class): {:#x?} ({:#X?})", dev, class);
    println!("@VGA configuring pcie device (class): {:#X?}", class);
    print!("@BOTH");

    let res_msi = capabilities::msi::init_msi_interrupt(FullPciDevType::Legacy(&dev));
    if let Err(e) = res_msi {
        println!("MSI init error: {:?}, trying MSIX", e);
        let res_msix = capabilities::msix::ini_msix_interrupt(FullPciDevType::Legacy(&dev));
        if let Err(e) = res_msix {
            println!("MSIX init error: {:?}, skipping device", e);
            return;
        }
    }

    let mut identification = DeviceIdentification::new(
        config_space::get_vendor_id(&dev.device),
        config_space::get_device_id(&dev.device),
        config_space::get_subsystem_vendor_id(&dev.device),
        config_space::get_subsystem_id(&dev.device),
    );

    crate::drivers::pci::device_codes::get_device_identification(&mut identification);
    println!("@DBG init_pci_device: Device identification: {:#X?}", identification);
    println!("@VGA init_pci_device: Vendor name: {}", identification.vendor_name);
    println!("@VGA init_pci_device: Device name: {}", identification.device_name);
    println!("@VGA init_pci_device: Subsys name: {}", identification.subsystem_name);
    print!("@BOTH");

    if matches!(
        class,
        device_class::PciClass::MassStorageController(MassStorageController::SerialATAController)
    ) {
        let ahci_disk = AhciController::new(dev);
        let ports = ahci_disk.init();
        for port in ports {
            block_task(Box::pin(crate::vfs::add_disk(Box::new(port))));
        }
    }

    printlnc!((51, 153, 10), "Device configured");
}

fn scan_pci_bus() -> Vec<PciDevice> {
    let mut devices = Vec::new();
    for bus in 0..=255 {
        for device in 0..32 {
            for function in 0..8 {
                //all IDs are valid
                let device = unsafe { PciDevice::new(None, bus, device, function).unwrap_unchecked() };
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

