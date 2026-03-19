use std::vec::Vec;

use crate::drivers::pci::{
    Capability, InterruptType, PciClass, PciDeviceLocation, PciDeviceNumericId, device_codes::DeviceIdentification,
};

#[derive(Debug)]
pub(super) struct CommonInfo {
    pub class: PciClass,
    pub identification: PciDeviceNumericId,
    pub identification_strings: DeviceIdentification,
    pub device: PciDeviceLocation,
    pub capabilities: Vec<Capability>,
    pub int_type: InterruptType,
}
