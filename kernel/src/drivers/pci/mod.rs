use crate::{drivers::pci::{
    capabilities::{Capability, msi, msix::{self, PCI_CAP_MSIX_ID}}, common_info::CommonInfo, driver_store::{PciDriverFactory, get_pci_driver_factory}
}, handler, interrupts};
use core::fmt::Debug;
use std::{boxed::Box, collections::btree_map::BTreeMap, error::ErrorCode, print, println, printlnc, r_lock_w_info, sync::rw_lock::RWSpinlock, vec::Vec, w_lock_w_info};

use crate::interrupts::{InterruptProcessorState, handlers::apic_eoi};

mod bar;
mod capabilities;
mod device_class;
mod device_codes;
mod driver_store;
mod express;
mod legacy;
mod port_access;
mod common_info;

pub use bar::*;
pub(super) use device_class::*;
pub(super) use device_codes::PciDeviceNumericId;
pub use express::express_device::PcieDevice;
pub use legacy::legacy_device::LegacyPciDevice;

pub(super) const PCI_CAP_POWER_MANAGEMENT_ID: u8 = 0x1;
pub(super) const PCI_CAP_PCIE_ID: u8 = 0x10;

pub use driver_store::{register_legacy_pci_driver, register_express_pci_driver};
pub use legacy::LegacyPciDriver;
pub use express::PcieDriver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PciDeviceLocation {
    group: u16,
    bus: u8,
    device: u8,
    function: u8,
}

impl PciDeviceLocation {
    pub fn new(group: Option<u16>, bus: u8, device: u8, function: u8) -> Result<Self, ErrorCode> {
        if device > 31 || function > 7 {
            return Err(ErrorCode::InvalidArgument);
        }
        Ok(Self {
            group: group.unwrap_or(0),
            bus,
            device,
            function,
        })
    }

    pub const fn empty() -> Self {
        Self {
            group: u16::MAX,
            bus: u8::MAX,
            device: u8::MAX,
            function: u8::MAX,
        }
    }
}

enum FullPciDevType {
    Legacy(LegacyPciDevice, Box<dyn LegacyPciDriver>),
    Express(PcieDevice, Box<dyn PcieDriver>),
}

impl FullPciDevType {
    fn get_common(&self) -> &CommonInfo {
        match self {
            Self::Legacy(dev, _) => &dev.common,
            Self::Express(dev, _) => &dev.common,
        }
    }

    fn get_common_mut(&mut self) -> &mut CommonInfo {
        match self {
            Self::Legacy(dev, _) => &mut dev.common,
            Self::Express(dev, _) => &mut dev.common,
        }
    }

    fn get_capabilities(&self) -> &Vec<Capability> {
        &self.get_common().capabilities
    }

    fn set_int_type(&mut self, int_type: InterruptType) {
        self.get_common_mut().int_type = int_type;       
    }

    fn get_int_type(&self) -> InterruptType {
        self.get_common().int_type
    }

    fn get_int_vector(&self) -> Option<u8> {
        match self.get_int_type() {
            InterruptType::Uninitialized => None,
            InterruptType::MSI => Some(msi::get_vector(self)),
            InterruptType::MSIX => Some(msix::get_vector(self)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum InterruptType {
    Uninitialized,
    MSI,
    MSIX,
}

static PCI_DEVICES: RWSpinlock<BTreeMap<PciDeviceLocation, RWSpinlock<FullPciDevType>>> = RWSpinlock::new(BTreeMap::new());

pub fn enumerate_devices() {
    let legacy_devices = legacy::get_devices();
    let express_devices = express::get_devices();

    for device in legacy_devices {
        println!("@DBG configuring pci device (class): {:#x?} ({:#X?})", device, device.common.class.clone());
        println!("@VGA configuring pci device, class: {:#X?}", device.common.class.clone());
        print!("@BOTH");
        let Some(PciDriverFactory::Legacy(driver_fn)) = get_pci_driver_factory(device.common.class.clone(), &device.common.identification) else {
            print!("No PCI driver loaded for device ");
            println!("@DBG {:#X?}", device.common.identification_strings);
            println!("@BOTH at {:?}", device.common.device);
            continue;
        };
        let driver = driver_fn();
        let location = device.common.device;
        let full_dev = RWSpinlock::new(FullPciDevType::Legacy(device, driver));
        let mut device_map = w_lock_w_info!(PCI_DEVICES);
        device_map.insert(location, full_dev);

        printlnc!((51, 153, 10), "Device configured");
    }

    for device in express_devices {
        println!("@DBG configuring pcie device (class): {:#x?} ({:#X?})", device, device.common.class.clone());
        println!("@VGA configuring pcie device, class: {:#X?}", device.common.class.clone());
        print!("@BOTH");
        let Some(PciDriverFactory::Express(driver_fn)) = get_pci_driver_factory(device.common.class.clone(), &device.common.identification) else {
            print!("No PCI driver loaded for device ");
            println!("@DBG {:#X?}", device.common.identification_strings);
            println!("@BOTH at {:?}", device.common.device);
            continue;
        };
        let driver = driver_fn();
        let location = device.common.device;
        let full_dev = RWSpinlock::new(FullPciDevType::Express(device, driver));
        let mut device_map = w_lock_w_info!(PCI_DEVICES);
        device_map.insert(location, full_dev);

        printlnc!((51, 153, 10), "Device configured");
    }

    let pci_devs = r_lock_w_info!(PCI_DEVICES);
    pci_devs.values().for_each(|dev| {
        let dev = &mut *w_lock_w_info!(dev);
        common_pci_config(dev);
        match dev {
            FullPciDevType::Legacy(device, driver) => driver.init(device),
            FullPciDevType::Express(device, driver) => driver.init(device),
        }
    });
}

fn common_pci_config(dev: &mut FullPciDevType) {
    let res_msi = capabilities::msi::init_msi_interrupt(dev);
    if let Err(e) = res_msi {
        println!("MSI init error: {:?}, trying MSIX", e);
        let res_msix = capabilities::msix::ini_msix_interrupt(dev);
        if let Err(e) = res_msix {
            println!("MSIX init error: {:?}, skipping device", e);
            return;
        } else {
            dev.set_int_type(InterruptType::MSIX);
        }
    } else {
        dev.set_int_type(InterruptType::MSI);
        let has_msi_x = dev.get_capabilities().iter().any(|cap| cap.id == PCI_CAP_MSIX_ID);
        println!("@DBG MSI init success, has_msix={}", has_msi_x);
        println!("@BOTH");
    }
}

fn set_interrupt_stub(index: u8) {
    extern "C" fn pci_stub() {
        println!("PCI interrupt, driver hasn't registered its handler yet");
        apic_eoi();
    }

    unsafe { interrupts::idt::IDT.set(interrupts::idt::Entry::new(handler!(pci_stub)), index.into()) };

}

fn set_interrupt_handler(_index: u8, _device_location: PciDeviceLocation) {
    todo!()
}
