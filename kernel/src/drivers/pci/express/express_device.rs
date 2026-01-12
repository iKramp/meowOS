use core::fmt::Debug;
use std::{mem_utils::VirtAddr, println, vec::Vec};

use crate::drivers::pci::{
    MemoryBar, PciDevice,
    capabilities::{Capability, ExtendedCapability},
    express::{LegacyConfigSpaceT0, LegacyConfigSpaceT0Ptr, configuration_space},
};

pub struct PcieDevice {
    device: PciDevice,
    pub(in crate::drivers::pci) config_space_addr: configuration_space::LegacyConfigSpaceT0Ptr<'static>,
    pub bars: Vec<MemoryBar>,
    pub(in crate::drivers::pci) capabilities: Vec<Capability>,
    pub(in crate::drivers::pci) extended_capabilities: Vec<ExtendedCapability>,
}

impl PcieDevice {
    pub(super) fn new(device: PciDevice, config_space_addr: VirtAddr) -> Self {
        Self {
            device,
            bars: Vec::new(),
            config_space_addr: unsafe { LegacyConfigSpaceT0Ptr::from_ptr(config_space_addr.0 as *mut LegacyConfigSpaceT0) },
            capabilities: Vec::new(),
            extended_capabilities: Vec::new(),
        }
    }

    pub(super) fn load_bars(&mut self) {
        let mut bars = Vec::new();

        let mut i = 0;
        while i < 6 {
            let bar = configuration_space::get_bar(self, i as u8);
            let Some(bar) = bar else {
                i += 1;
                continue;
            };
            println!("Bar {}: {:#X?}", i, bar);
            i += if bar.is_64_bit { 2 } else { 1 };
            bars.push(bar);
        }
        self.bars = bars;
    }

    pub(super) fn load_capabilities(&mut self) {
        self.capabilities = configuration_space::get_capabilities_list(self);
    }

    pub(super) fn load_extended_capabilities(&mut self) {
        self.extended_capabilities = configuration_space::get_extended_capabilities_list(self);
    }
}

impl Debug for PcieDevice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PcieDevice")
            .field("device", &self.device)
            .field("config_space_addr", &self.config_space_addr.as_ptr())
            .field("bars", &self.bars)
            .field("capabilities", &self.capabilities)
            .field("extended_capabilities", &self.extended_capabilities)
            .finish()
    }
}
