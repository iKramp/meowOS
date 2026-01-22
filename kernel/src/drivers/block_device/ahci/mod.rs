use std::boxed::Box;

use crate::drivers::pci::{MassStorageController, PciClass};

pub mod disk;
mod fis;

pub(super) fn init_driver() {
    crate::drivers::pci::register_legacy_pci_driver(
        PciClass::MassStorageController(MassStorageController::SerialATAController),
        None,
        None,
        None,
        None,
        || Box::new(disk::AhciDriver),
    );
}
