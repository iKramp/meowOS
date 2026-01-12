use crate::drivers::pci::{MassStorageController, PciClass, PciDeviceNumericId};

pub mod disk;
mod fis;

pub(super) fn init_driver() {
    crate::drivers::pci::add_legacy_pci_driver((
        PciClass::MassStorageController(MassStorageController::SerialATAController),
        PciDeviceNumericId {
            vendor_id: None,
            device_id: None,
            subvendor_id: None,
            subdevice_id: None,
        }
    ), disk::ahci_driver_init);
}
