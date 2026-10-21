use std::{boxed::Box, println, sync::rw_lock::RWSpinlock, vec::Vec, w_lock_w_info};

use crate::drivers::pci::{
    LegacyPciDriver, PciClass, PciDeviceNumericId, PcieDriver, device_codes::PciDeeviceTypeIdentification,
};

#[derive(Clone, Copy)]
pub(super) enum PciDriverFactory {
    Legacy(fn() -> Box<dyn LegacyPciDriver>),
    Express(fn() -> Box<dyn PcieDriver>),
}

type PciDriverEntry = (PciDeeviceTypeIdentification, PciDriverFactory);
static PCI_DEVICE_DRIVERS: RWSpinlock<Vec<PciDriverEntry>> = RWSpinlock::new(Vec::new());

pub fn register_legacy_pci_driver(
    class: Option<PciClass>,
    vendor_id: Option<u16>,
    device_id: Option<u16>,
    subvendor_id: Option<u16>,
    subdevice_id: Option<u16>,
    factory: fn() -> Box<dyn LegacyPciDriver>,
) {
    register_pci_driver(
        class,
        PciDeviceNumericId {
            vendor_id,
            device_id,
            subvendor_id,
            subdevice_id,
        },
        PciDriverFactory::Legacy(factory),
    );
}

pub fn register_express_pci_driver(
    class: Option<PciClass>,
    vendor_id: Option<u16>,
    device_id: Option<u16>,
    subvendor_id: Option<u16>,
    subdevice_id: Option<u16>,
    factory: fn() -> Box<dyn PcieDriver>,
) {
    register_pci_driver(
        class,
        PciDeviceNumericId {
            vendor_id,
            device_id,
            subvendor_id,
            subdevice_id,
        },
        PciDriverFactory::Express(factory),
    );
}

fn get_pci_driver_factory_by_class_and_id(
    drivers: &[PciDriverEntry],
    target_identification: &PciDeeviceTypeIdentification,
) -> Option<PciDriverFactory> {
    drivers
        .iter()
        .find(|(identification, _)| target_identification == identification)
        .map(|(_, factory)| *factory)
}

fn register_pci_driver(class: Option<PciClass>, id: PciDeviceNumericId, factory: PciDriverFactory) {
    if class.is_none() && id.is_empty() {
        return; // Don't register a driver with no class and no ID
    }

    let key = PciDeeviceTypeIdentification {
        class: class.clone(),
        id,
    };
    let mut drivers = w_lock_w_info!(PCI_DEVICE_DRIVERS);
    let existing = get_pci_driver_factory_by_class_and_id(&drivers, &key);
    if existing.is_some() {
        return; // Don't register a driver if one already exists for the same class and ID
    }
    drivers.push((key, factory));
}

pub fn get_pci_driver_factory(class: PciClass, numeric_id: &PciDeviceNumericId) -> Option<PciDriverFactory> {
    let identification = PciDeeviceTypeIdentification {
        class: Some(class),
        id: numeric_id.clone(),
    };
    let drivers = w_lock_w_info!(PCI_DEVICE_DRIVERS);
    get_pci_driver_factory_by_class_and_id(&drivers, &identification)
}

pub fn print_all_pci_drivers() {
    let drivers = w_lock_w_info!(PCI_DEVICE_DRIVERS);
    println!("Registered PCI Drivers:");
    for (identification, factory) in drivers.iter() {
        let driver_type = match factory {
            PciDriverFactory::Legacy(_) => "Legacy",
            PciDriverFactory::Express(_) => "Express",
        };
        println!(
            "Class: {:?}, Vendor ID: {:?}, Device ID: {:?}, Subvendor ID: {:?}, Subdevice ID: {:?}, Type: {}",
            identification.class,
            identification.id.vendor_id,
            identification.id.device_id,
            identification.id.subvendor_id,
            identification.id.subdevice_id,
            driver_type
        );
    }
}
