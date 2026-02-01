use crate::{drivers::pci::{
    capabilities::{Capability, msi, msix::{self, PCI_CAP_MSIX_ID}}, common_info::CommonInfo, driver_store::{PciDriverFactory, get_pci_driver_factory}
}, handler, interrupts};
use core::{fmt::Debug, sync::atomic::{AtomicU8, Ordering}};
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
use unroll::unroll_for_loops;

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
}

#[derive(Debug, Clone, Copy)]
enum InterruptType {
    Uninitialized,
    Msi,
    MsiX,
}

static PCI_DEVICES: RWSpinlock<BTreeMap<PciDeviceLocation, RWSpinlock<FullPciDevType>>> = RWSpinlock::new(BTreeMap::new());

pub fn init() {
    init_interrupts();
    enumerate_devices();
}

fn enumerate_devices() {
    let legacy_devices = legacy::get_devices();
    let express_devices = express::get_devices();

    for device in legacy_devices {
        println!("configuring pci device (class): {:#x?} ({:#X?})", device, device.common.class.clone());
        let Some(PciDriverFactory::Legacy(driver_fn)) = get_pci_driver_factory(device.common.class.clone(), &device.common.identification) else {
            println!("No PCI driver loaded for device {:#X?}", device.common.identification_strings);
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
        println!("configuring pcie device (class): {:#x?} ({:#X?})", device, device.common.class.clone());
        let Some(PciDriverFactory::Express(driver_fn)) = get_pci_driver_factory(device.common.class.clone(), &device.common.identification) else {
            println!(level:info, "No PCI driver loaded for device {:#X?}", device.common.identification_strings);
            continue;
        };
        let driver = driver_fn();
        let location = device.common.device;
        let full_dev = RWSpinlock::new(FullPciDevType::Express(device, driver));
        let mut device_map = w_lock_w_info!(PCI_DEVICES);
        device_map.insert(location, full_dev);

        printlnc!((51, 153, 10), "Device configured");
    }

    let mut pci_devs = w_lock_w_info!(PCI_DEVICES);
    let mut to_remove = Vec::new();
    pci_devs.values().for_each(|dev| {
        let dev = &mut *w_lock_w_info!(dev);
        println!("Initializing PCI device at {:?}", dev.get_common().device);
        common_pci_config(dev);
        let result = match dev {
            FullPciDevType::Legacy(device, driver) => driver.init(device),
            FullPciDevType::Express(device, driver) => driver.init(device),
        };
        if let Err(e) = result {
            println!(level:error, "PCI device {:?} deriver errored during initialization: {e}. Uninitializing...", dev.get_common().device);
            to_remove.push(dev.get_common().device);
            if let Err(e) = common_pci_unconfig(dev) {
                println!(level:error, "error while uninitializing device: {e}")
            }
        }
    });
    for dev_to_remove in to_remove {
        pci_devs.remove(&dev_to_remove);
    }
}

fn common_pci_config(dev: &mut FullPciDevType) {
    let irq = alocate_irq();
    w_lock_w_info!(PCI_INTERRUPT_HANDLERS)[irq as usize].push(dev.get_common().device);
    let Err(e_msi) = capabilities::msi::init_msi_interrupt(dev, irq + 128) else {
        dev.set_int_type(InterruptType::Msi);
        println!("MSI init success");
        return;
    };
    let Err(e_msix) = capabilities::msix::ini_msix_interrupt(dev, irq + 128) else {
        dev.set_int_type(InterruptType::MsiX);
        println!("MSIX init success");
        return;
    };
    w_lock_w_info!(PCI_INTERRUPT_HANDLERS)[irq as usize].retain(|&loc| loc != dev.get_common().device);
    println!("MSI/MSIX init error: {:?}/{:?}, skipping device interrupt setup", e_msi, e_msix);
}

fn common_pci_unconfig(dev: &mut FullPciDevType) -> Result<(), ErrorCode> {
    match dev.get_int_type() {
        InterruptType::Uninitialized => Ok(()),
        InterruptType::Msi => msi::disable_msi(dev),
        InterruptType::MsiX => msix::disable_msix(dev)
    }
}

static PCI_INTERRUPT_HANDLERS: RWSpinlock<[Vec<PciDeviceLocation>; 32]> = RWSpinlock::new([const { Vec::new() }; 32]);
static PCI_INT_IRQ: AtomicU8 = AtomicU8::new(0);

fn alocate_irq() -> u8 {
    PCI_INT_IRQ.fetch_add(1, Ordering::SeqCst) % 32
}

fn common_pci_interrupt_handler(irq_index: u8) {
    let pci_devs_lock = r_lock_w_info!(PCI_INTERRUPT_HANDLERS);
    let pci_devs = pci_devs_lock[irq_index as usize].clone();
    drop(pci_devs_lock);
    for dev_location in pci_devs.iter() {
        let pci_devices_lock = r_lock_w_info!(PCI_DEVICES);
        let pci_dev_lock = match pci_devices_lock.get(dev_location) {
            Some(dev) => dev,
            None => {
                continue;
            }
        };
        let mut pci_dev = w_lock_w_info!(pci_dev_lock);
        match &mut *pci_dev {
            FullPciDevType::Legacy(device, driver) => {
                driver.service_interrupt(device);
            }
            FullPciDevType::Express(device, driver) => {
                driver.service_interrupt(device);
            }
        }
    }
    apic_eoi();
}

#[unroll_for_loops]
fn init_interrupts() {
    for i in 128..160 {
        {
            extern "C" fn pci_interrupt_handler(_state: &mut InterruptProcessorState) {
                common_pci_interrupt_handler((i - 128) as u8);
            }
            unsafe { interrupts::idt::IDT.set(interrupts::idt::Entry::new(handler!(pci_interrupt_handler)), i) };
        }
    }
}
