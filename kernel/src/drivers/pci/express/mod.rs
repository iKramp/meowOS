use std::{
    mem_utils::{PhysAddr, VirtAddr, translate_phys_virt_addr},
    print, println, vec::Vec,
};

use crate::{
    acpi::{BaseAddressAllocation, McfgTable},
    drivers::pci::{
        PCI_CAP_PCIE_ID, PCI_CAP_POWER_MANAGEMENT_ID, PciDeviceLocation, device_class::PciClass, device_codes::PciDeviceNumericId, express::{
            configuration_space::{LegacyConfigSpaceT0, LegacyConfigSpaceT0Ptr},
            express_device::PcieDevice,
        }
    },
    memory::{PAGE_TREE_ALLOCATOR, paging::LiminePat},
};

mod configuration_space;
pub(super) mod express_device;

type PcieDevDriverInitFn = ((PciClass, PciDeviceNumericId), fn(PcieDevice));

pub trait PcieDriver: Send + Sync {
    fn init(&mut self, dev: &PcieDevice);
    fn deinit(&mut self, dev: &PcieDevice);
    /// Called after the device is removed from the system
    /// Either forcibly, or deinit was called earlier
    fn remove_device(&mut self);
    fn service_interrupt(&mut self, dev: &PcieDevice);
}

pub fn get_devices() -> Vec<PcieDevice> {
    let Some(mcfg_table) = crate::acpi::get_table::<McfgTable>("MCFG") else {
        println!("No MCFG table found, skipping PCIe initialization");
        return Vec::new();
    };

    println!("@DBG pci::enumerate_devices: MCFG table found at {:#?}", mcfg_table);
    print!("@BOTH");

    let pcie_allocations = mcfg_table.allocations();

    println!(
        "pci::enumerate_devices: Found {} PCIe allocations in MCFG table",
        pcie_allocations.len()
    );
    scan_pcie_bus(&pcie_allocations)
}

fn scan_pcie_bus(allocations: &[&'static BaseAddressAllocation]) -> Vec<PcieDevice> {
    let mut devices = Vec::new();
    for allocation in allocations.iter() {
        println!("Scanning PCIe bus from {:x?}", allocation);
        for bus in allocation.start_bus_number()..allocation.end_bus_number() {
            for device in 0..32 {
                for function in 0..8 {
                    if let Some(dev) = check_pcie_device(allocation, bus, device, function) {
                        devices.push(dev);
                    }
                }
            }
        }
    }

    devices
}

fn check_pcie_device(allocation: &BaseAddressAllocation, bus: u8, device: u8, function: u8) -> Option<PcieDevice> {
    //get config space
    let phys_addr = allocation.base_address() + ((bus as u64 * 256) + (device as u64 * 8) + function as u64) * 0x1000;
    let addr = translate_phys_virt_addr(phys_addr);
    let config_space_ptr = unsafe { LegacyConfigSpaceT0Ptr::from_ptr(addr.0 as *mut LegacyConfigSpaceT0) };
    if config_space_ptr.vendor_id().read() == 0xFFFF {
        return None;
    }
    let pci_device = PciDeviceLocation::new(Some(allocation.pci_segment_group_number()), bus, device, function)
        .expect("loops have correct bounds");
    let mut pcie_device = PcieDevice::new(pci_device, addr);
    pcie_device.load_capabilities();

    //check capabilities
    if !is_pcie(&mut pcie_device) {
        return None;
    }

    pcie_device.load_bars();
    pcie_device.load_extended_capabilities();

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
    Some(pcie_device)
}

fn is_pcie(device: &mut PcieDevice) -> bool {
    //check capabilities
    let caps = &device.common.capabilities;
    let has_pcie_cap = caps.iter().any(|cap| cap.id == PCI_CAP_PCIE_ID);
    let has_power_mgmt_cap = caps.iter().any(|cap| cap.id == PCI_CAP_POWER_MANAGEMENT_ID);
    if !has_pcie_cap {
        println!("@DBG Skipping non-PCIe device{:?}", device);
        print!("@BOTH");
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
