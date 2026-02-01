use core::fmt::Debug;
use std::{mem_utils::VirtAddr, print, println, vec::Vec};

use crate::drivers::pci::{
    InterruptType, MemoryBar, PciClass, PciDeviceLocation, PciDeviceNumericId, capabilities::ExtendedCapability, common_info::CommonInfo, express::{LegacyConfigSpaceT0, LegacyConfigSpaceT0Ptr, configuration_space}
};

pub struct PcieDevice {
    pub(in crate::drivers::pci) config_space_addr: configuration_space::LegacyConfigSpaceT0Ptr<'static>,
    pub bars: Vec<MemoryBar>,
    pub(in crate::drivers::pci) extended_capabilities: Vec<ExtendedCapability>,
    pub(in crate::drivers::pci) common: CommonInfo,
}

impl PcieDevice {
    pub(super) fn new(device: PciDeviceLocation, config_space_addr: VirtAddr) -> Self {
        let config_space_addr = unsafe { LegacyConfigSpaceT0Ptr::from_ptr(config_space_addr.0 as *mut LegacyConfigSpaceT0) };

        let class = PciClass::from(config_space_addr.class_code().read(), config_space_addr.subclass().read());

        let identification = PciDeviceNumericId {
            vendor_id: Some(config_space_addr.vendor_id().read()),
            device_id: Some(config_space_addr.device_id().read()),
            subvendor_id: Some(config_space_addr.subsystem_vendor_id().read()),
            subdevice_id: Some(config_space_addr.subsystem_id().read()),
        };
        let identification_strings = crate::drivers::pci::device_codes::get_device_identification(identification.clone());
        println!("init_pci_device: Device identification: {:#X?}", identification_strings);

        Self {
            bars: Vec::new(),
            config_space_addr,
            extended_capabilities: Vec::new(),
            common: CommonInfo {
                class,
                identification,
                identification_strings,
                device,
                capabilities: Vec::new(),
                int_type: InterruptType::Uninitialized,
            },
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
        self.common.capabilities = configuration_space::get_capabilities_list(self);
    }

    pub(super) fn load_extended_capabilities(&mut self) {
        self.extended_capabilities = configuration_space::get_extended_capabilities_list(self);
    }

    pub fn enable_bus_mastering(&self) {
        let mut command = self.config_space_addr.command().read();
        self.config_space_addr.command().write(*command.set_bus_master_enable(true));
    }
}

impl Debug for PcieDevice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PcieDevice")
            .field("device", &self.common.device)
            .field("config_space_addr", &self.config_space_addr.as_ptr())
            .field("bars", &self.bars)
            .field("capabilities", &self.common.capabilities)
            .field("extended_capabilities", &self.extended_capabilities)
            .finish()
    }
}
