use std::{println, vec::Vec};

use crate::drivers::pci::{
    InterruptType, PciDeviceLocation, PciDeviceNumericId, bar::Bar, common_info::CommonInfo, legacy::config_space,
};

#[derive(Debug)]
pub struct LegacyPciDevice {
    pub bars: Vec<Bar>,
    pub(in crate::drivers::pci) common: CommonInfo,
}

impl LegacyPciDevice {
    pub(super) fn new(device: PciDeviceLocation) -> Self {
        let mut bars = Vec::new();
        let mut i = 0;

        //disconnect device from any BARs
        let command = config_space::get_command(&device);
        config_space::set_command(command & !0x3, &device);

        while i < 6 {
            let bar = config_space::get_bar(i, &device);
            if let Some(bar) = bar {
                bars.push(bar.0);
                i += bar.1;
            } else {
                i += 1;
            }
        }
        config_space::set_command(command, &device);

        let class = config_space::get_class(&device);

        let identification = PciDeviceNumericId {
            vendor_id: Some(config_space::get_vendor_id(&device)),
            device_id: Some(config_space::get_device_id(&device)),
            subvendor_id: Some(config_space::get_subsystem_vendor_id(&device)),
            subdevice_id: Some(config_space::get_subsystem_id(&device)),
        };
        let identification_strings = crate::drivers::pci::device_codes::get_device_identification(identification.clone());
        println!("init_pci_device: Device identification: {:#X?}", identification_strings);

        let mut dev = Self {
            bars,
            common: CommonInfo {
                class,
                identification,
                identification_strings,
                device,
                capabilities: Vec::new(),
                int_type: InterruptType::Uninitialized,
            },
        };
        config_space::load_capabilities_list(&mut dev);
        dev
    }

    pub fn enable_bus_mastering(&self) {
        let command = config_space::get_command(&self.common.device);
        config_space::set_command(command | 0b100, &self.common.device);
    }
}
