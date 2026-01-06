use std::vec::Vec;

use crate::drivers::pci::{PciDevice, bar::Bar, capabilities::Capability, legacy::config_space};

#[derive(Debug)]
pub struct LegacyPciDevice {
    pub(in crate::drivers::pci) device: PciDevice,
    pub bars: Vec<Bar>,
    pub(in crate::drivers::pci) capabilities: Vec<Capability>,
}

impl LegacyPciDevice {
    pub(super) fn new(device: PciDevice) -> Self {
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
        let mut dev = Self {
            device,
            bars,
            capabilities: Vec::new(),
        };
        config_space::load_capabilities_list(&mut dev);
        dev
    }

    pub fn enable_bus_mastering(&self) {
        let command = config_space::get_command(&self.device);
        config_space::set_command(command | 0b100, &self.device);
    }
}
