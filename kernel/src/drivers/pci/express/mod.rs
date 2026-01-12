use std::{
    lock_w_info,
    mem_utils::{PhysAddr, VirtAddr, translate_phys_virt_addr},
    print, println, printlnc,
    sync::no_int_spinlock::NoIntSpinlock,
    vec::Vec,
};

use crate::{
    acpi::{BaseAddressAllocation, McfgTable},
    drivers::pci::{
        FullPciDevType, PCI_CAP_PCIE_ID, PCI_CAP_POWER_MANAGEMENT_ID, PciDevice, capabilities::{self, msix::PCI_CAP_MSIX_ID},
        device_class::PciClass,
        device_codes::PciDeviceNumericId,
        express::{
            configuration_space::{LegacyConfigSpaceT0, LegacyConfigSpaceT0Ptr},
            express_device::PcieDevice,
        },
    },
    memory::{PAGE_TREE_ALLOCATOR, paging::LiminePat},
};

mod configuration_space;
pub mod express_device;

type PcieDevDriverInitFn = ((PciClass, PciDeviceNumericId), fn(PcieDevice));

static LEGACY_PCI_DRIVERS: NoIntSpinlock<Vec<PcieDevDriverInitFn>> = NoIntSpinlock::new(Vec::new());

pub(in crate::drivers) fn add_pcie_driver(dev_type: (PciClass, PciDeviceNumericId), init_fn: fn(PcieDevice)) {
    let mut drivers = lock_w_info!(LEGACY_PCI_DRIVERS);
    drivers.push((dev_type, init_fn));
}

pub fn configure_devices() {
    let Some(mcfg_table) = crate::acpi::get_table::<McfgTable>("MCFG") else {
        println!("No MCFG table found, skipping PCIe initialization");
        return;
    };

    println!("@DBG pci::enumerate_devices: MCFG table found at {:#?}", mcfg_table);
    print!("@BOTH");

    let pcie_allocations = mcfg_table.allocations();

    println!(
        "pci::enumerate_devices: Found {} PCIe allocations in MCFG table",
        pcie_allocations.len()
    );
    let devices = scan_pcie_bus(&pcie_allocations);
    println!("pci::enumerate_devices: Found {} PCIe devices", devices.len());

    for device in devices {
        init_pci_device(device);
    }
}

pub fn init_pci_device(mut dev: PcieDevice) {
    let class = PciClass::from(
        dev.config_space_addr.class_code().read(),
        dev.config_space_addr.subclass().read(),
    );
    if dev.config_space_addr.header_type().read().header_type() != 0 {
        println!("skipping device {:#X?} because i don't configure bridges yet", class);
        return;
    }
    println!("@DBG configuring pcie device (class): {:#x?} ({:#X?})", dev, class);
    println!("@VGA configuring pcie device (class): {:#X?}", class);
    print!("@BOTH");
    dev.load_bars();

    let res_msi = capabilities::msi::init_msi_interrupt(FullPciDevType::Express(&dev));
    if let Err(e) = res_msi {
        println!("MSI init error: {:?}, trying MSIX", e);
        let res_msix = capabilities::msix::ini_msix_interrupt(FullPciDevType::Express(&dev));
        if let Err(e) = res_msix {
            println!("MSIX init error: {:?}, skipping device", e);
            return;
        }
    } else {
        let has_msi_x = dev
            .capabilities
            .iter()
            .any(|cap| cap.id == PCI_CAP_MSIX_ID);
        println!("@DBG MSI init success, has_msix={}", has_msi_x);
        println!("@BOTH");
    }

    let identification = PciDeviceNumericId {
        vendor_id: Some(dev.config_space_addr.vendor_id().read()),
        device_id: Some(dev.config_space_addr.device_id().read()),
        subvendor_id: Some(dev.config_space_addr.subsystem_vendor_id().read()),
        subdevice_id: Some(dev.config_space_addr.subsystem_id().read()),
    };

    let identification = crate::drivers::pci::device_codes::get_device_identification(identification);
    println!("@DBG init_pci_device: Device identification: {:#X?}", identification);
    println!("@VGA init_pci_device: Vendor name: {}", identification.vendor_name);
    println!("@VGA init_pci_device: Device name: {}", identification.device_name);
    println!("@VGA init_pci_device: Subsys name: {}", identification.subsystem_name);
    print!("@BOTH");

    if let Some(driver_init_fn) = find_driver(class.clone(), &identification.id) {
        println!("init_pci_device: Found driver for device {:#X?}", class);
        driver_init_fn(dev);
    } else {
        println!("init_pci_device: No driver found for device {:#X?}", class);
    }

    printlnc!((51, 153, 10), "Device configured");
}

fn scan_pcie_bus(allocations: &[&'static BaseAddressAllocation]) -> Vec<PcieDevice> {
    let mut devices = Vec::new();
    for allocation in allocations.iter() {
        println!("Scanning PCIe bus from {:x?}", allocation);
        for bus in allocation.start_bus_number()..allocation.end_bus_number() {
            for device in 0..32 {
                for function in 0..8 {
                    //get config space
                    let phys_addr =
                        allocation.base_address() + ((bus as u64 * 256) + (device as u64 * 8) + function as u64) * 0x1000;
                    let addr = translate_phys_virt_addr(phys_addr);
                    let config_space_ptr = unsafe { LegacyConfigSpaceT0Ptr::from_ptr(addr.0 as *mut LegacyConfigSpaceT0) };
                    if config_space_ptr.vendor_id().read() == 0xFFFF {
                        continue;
                    }
                    let pci_device = PciDevice::new(Some(allocation.pci_segment_group_number()), bus, device, function)
                        .expect("loops have correct bounds");
                    let mut pcie_device = PcieDevice::new(pci_device, addr);
                    pcie_device.load_capabilities();

                    //check capabilities
                    if !is_pcie(&mut pcie_device) {
                        continue;
                    }

                    //above is fine from cached data, it's not modifying anything
                    let config_space_address = map_config_space(phys_addr);
                    pcie_device.config_space_addr =
                        unsafe { LegacyConfigSpaceT0Ptr::from_ptr(config_space_address.0 as *mut LegacyConfigSpaceT0) };

                    //get device type
                    let class = config_space_ptr.class_code().read();
                    let subclass = config_space_ptr.subclass().read();
                    let class = PciClass::from(class, subclass);
                    println!("@DBG Found PCIe device: {:#X?}, class: {:?}", pcie_device, class);
                    println!("@VGA Found PCIe device (class: {:?})", class);
                    devices.push(pcie_device);
                }
            }
        }
    }

    devices
}

fn is_pcie(device: &mut PcieDevice) -> bool {
    //check capabilities
    let caps = &device.capabilities;
    let has_pcie_cap = caps.iter().any(|cap| cap.id == PCI_CAP_PCIE_ID);
    let has_power_mgmt_cap = caps.iter().any(|cap| cap.id == PCI_CAP_POWER_MANAGEMENT_ID);
    if !has_pcie_cap {
        println!("Skipping non-PCIe device{:?}", device);
        return false;
    }
    assert!(
        has_power_mgmt_cap,
        "PCIe device without power management capability found: {:#X?}",
        device
    );
    true
}

fn map_config_space(phys_addr: PhysAddr) -> VirtAddr {
    let pci_dev_virt = unsafe { PAGE_TREE_ALLOCATOR.allocate(Some(phys_addr), false) };
    let page_entry = unsafe {
        PAGE_TREE_ALLOCATOR
            .get_page_table_entry_mut(pci_dev_virt)
            .expect("just allocated")
    };
    page_entry.set_pat(LiminePat::UC);

    pci_dev_virt
}

fn find_driver(dev_class: PciClass, identification: &PciDeviceNumericId) -> Option<fn(PcieDevice)> {
    let drivers = lock_w_info!(LEGACY_PCI_DRIVERS);
    for ((driver_dev_class, driver_dev_id), driver) in drivers.iter() {
        if dev_class != *driver_dev_class {
            continue;
        }
        let matches_vendor = match identification.vendor_id {
            Some(vendor_id) => match driver_dev_id.vendor_id {
                Some(driver_vendor_id) => vendor_id == driver_vendor_id,
                None => true,
            },
            None => true,
        };
        let matches_device = match identification.device_id {
            Some(device_id) => match driver_dev_id.device_id {
                Some(driver_device_id) => device_id == driver_device_id,
                None => true,
            },
            None => true,
        };
        let matches_subvendor = match identification.subvendor_id {
            Some(subvendor_id) => match driver_dev_id.subvendor_id {
                Some(driver_subvendor_id) => subvendor_id == driver_subvendor_id,
                None => true,
            },
            None => true,
        };
        let matches_subdevice = match identification.subdevice_id {
            Some(subdevice_id) => match driver_dev_id.subdevice_id {
                Some(driver_subdevice_id) => subdevice_id == driver_subdevice_id,
                None => true,
            },
            None => true,
        };
        if matches_vendor && matches_device && matches_subvendor && matches_subdevice {
            return Some(*driver);
        }
    }
    None
}
