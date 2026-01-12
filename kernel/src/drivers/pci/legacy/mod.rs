use std::{lock_w_info, print, println, printlnc, sync::no_int_spinlock::NoIntSpinlock, vec::Vec};

use crate::{
    drivers::pci::{
        FullPciDevType, LegacyPciDevice, PCI_CAP_PCIE_ID, PciDevice, capabilities, device_class::PciClass,
        device_codes::PciDeviceNumericId, port_access,
    },
};

pub(super) mod config_space;
pub(super) mod legacy_device;

type LegacyPciDevDriverInitFn = ((PciClass, PciDeviceNumericId), fn(LegacyPciDevice));

static LEGACY_PCI_DRIVERS: NoIntSpinlock<Vec<LegacyPciDevDriverInitFn>> =
    NoIntSpinlock::new(Vec::new());

pub(in crate::drivers) fn add_legacy_pci_driver(dev_type: (PciClass, PciDeviceNumericId), init_fn: fn(LegacyPciDevice)) {
    let mut drivers = lock_w_info!(LEGACY_PCI_DRIVERS);
    drivers.push((dev_type, init_fn));
}

pub fn configure_devices() {
    let devices = scan_pci_bus();

    for device in devices {
        println!(
            "pci::enumerate_devices: Found device at {:02x}:{:02x}.{:x}",
            device.bus, device.device, device.function
        );

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

    let identification = PciDeviceNumericId {
        vendor_id: Some(config_space::get_vendor_id(&dev.device)),
        device_id: Some(config_space::get_device_id(&dev.device)),
        subvendor_id: Some(config_space::get_subsystem_vendor_id(&dev.device)),
        subdevice_id: Some(config_space::get_subsystem_id(&dev.device)),
    };

    let identification = crate::drivers::pci::device_codes::get_device_identification(identification);
    println!("@DBG init_pci_device: Device identification: {:#X?}", identification);
    println!("@VGA init_pci_device: Vendor name: {}", identification.vendor_name);
    println!("@VGA init_pci_device: Device name: {}", identification.device_name);
    println!("@VGA init_pci_device: Subsys name: {}", identification.subsystem_name);
    print!("@BOTH");

    if let Some(driver_init_fn) = find_driver(class.clone(), &identification.id) {
        println!("init_pci_device: Found driver for device {:#X?}", class);
        driver_init_fn(dev);
    } else {
        println!("init_pci_device: No driver found for device {:#X?}", class);
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

fn find_driver(dev_class: PciClass, identification: &PciDeviceNumericId) -> Option<fn(LegacyPciDevice)> {
    let drivers = lock_w_info!(LEGACY_PCI_DRIVERS);
    for ((driver_dev_class, driver_dev_id), driver) in drivers.iter() {
        if dev_class != *driver_dev_class {
            continue;
        }
        let matches_vendor = match identification.vendor_id {
            Some(vendor_id) => match driver_dev_id.vendor_id {
                Some(driver_vendor_id) => vendor_id == driver_vendor_id,
                None => true,
            },
            None => true,
        };
        let matches_device = match identification.device_id {
            Some(device_id) => match driver_dev_id.device_id {
                Some(driver_device_id) => device_id == driver_device_id,
                None => true,
            },
            None => true,
        };
        let matches_subvendor = match identification.subvendor_id {
            Some(subvendor_id) => match driver_dev_id.subvendor_id {
                Some(driver_subvendor_id) => subvendor_id == driver_subvendor_id,
                None => true,
            },
            None => true,
        };
        let matches_subdevice = match identification.subdevice_id {
            Some(subdevice_id) => match driver_dev_id.subdevice_id {
                Some(driver_subdevice_id) => subdevice_id == driver_subdevice_id,
                None => true,
            },
            None => true,
        };
        if matches_vendor && matches_device && matches_subvendor && matches_subdevice {
            return Some(*driver);
        }
    }
    None
}
