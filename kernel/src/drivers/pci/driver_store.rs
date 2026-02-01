use std::{boxed::Box, collections::btree_map::BTreeMap, sync::rw_lock::RWSpinlock, vec::Vec, w_lock_w_info};

use crate::drivers::pci::{LegacyPciDriver, PciClass, PciDeviceNumericId, PcieDriver};

#[derive(Clone, Copy)]
pub(super) enum PciDriverFactory {
    Legacy(fn() -> Box<dyn LegacyPciDriver>),
    Express(fn() -> Box<dyn PcieDriver>),
}

static PCI_DEVICE_DRIVERS: RWSpinlock<BTreeMap<PciClass, Vec<(PciDeviceNumericId, PciDriverFactory)>>> =
    RWSpinlock::new(BTreeMap::new());

pub fn register_legacy_pci_driver(
    class: PciClass,
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
    class: PciClass,
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

fn register_pci_driver(class: PciClass, id: PciDeviceNumericId, factory: PciDriverFactory) {
    let key = (class.clone(), id);
    let mut drivers = w_lock_w_info!(PCI_DEVICE_DRIVERS);
    let class_drivers = drivers.entry(class).or_default();
    class_drivers.push((key.1, factory));
}

pub fn get_pci_driver_factory(class: PciClass, numeric_id: &PciDeviceNumericId) -> Option<PciDriverFactory> {
    let drivers = w_lock_w_info!(PCI_DEVICE_DRIVERS);
    let class_drivers = drivers.get(&class)?;
    class_drivers
        .iter()
        .find(|(driver_id, _)| driver_id == numeric_id)
        .map(|(_, factory)| factory)
        .copied()
}
