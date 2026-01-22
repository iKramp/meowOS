use crate::{
    drivers::pci::{
        LegacyPciDevice, PciDeviceLocation, bar::{Bar, IOBar, MemoryBar}, capabilities::Capability, device_class::PciClass, port_access::{get_dword, set_dword}
    },
};
use std::{mem_utils::PhysAddr, println, vec::Vec};

pub fn get_vendor_id(dev: &PciDeviceLocation) -> u16 {
    get_dword(0, dev) as u16
}

pub fn get_device_id(dev: &PciDeviceLocation) -> u16 {
    (get_dword(0, dev) >> 16) as u16
}

pub fn get_subsystem_vendor_id(dev: &PciDeviceLocation) -> u16 {
    get_dword(0x2C, dev) as u16
}

pub fn get_subsystem_id(dev: &PciDeviceLocation) -> u16 {
    (get_dword(0x2C, dev) >> 16) as u16
}

pub fn get_command(dev: &PciDeviceLocation) -> u16 {
    get_dword(4, dev) as u16
}

pub fn set_command(value: u16, dev: &PciDeviceLocation) {
    set_dword(4, value as u32, dev);
}

pub fn get_status(dev: &PciDeviceLocation) -> u16 {
    (get_dword(4, dev) >> 16) as u16
}

pub fn get_revision_id(dev: &PciDeviceLocation) -> u8 {
    get_dword(8, dev) as u8
}

pub fn get_progif(dev: &PciDeviceLocation) -> u8 {
    (get_dword(8, dev) >> 8) as u8
}

pub fn get_class(dev: &PciDeviceLocation) -> PciClass {
    let class_subclass = get_dword(8, dev) >> 16;
    let class = (class_subclass >> 8) as u8;
    let subclass = class_subclass as u8;
    PciClass::from(class, subclass)
}

pub fn get_header_type(dev: &PciDeviceLocation) -> u8 {
    (get_dword(0xC, dev) >> 16) as u8
}

pub fn get_bist(dev: &PciDeviceLocation) -> u8 {
    (get_dword(0xC, dev) >> 24) as u8
}

pub fn get_latency_timer(dev: &PciDeviceLocation) -> u8 {
    (get_dword(0xC, dev) >> 8) as u8
}

pub fn get_cache_line_size(dev: &PciDeviceLocation) -> u8 {
    get_dword(0xC, dev) as u8
}

pub fn get_bar(index: u8, dev: &PciDeviceLocation) -> Option<(Bar, u8)> {
    #[cfg(debug_assertions)]
    {
        let header_type = get_header_type(dev) & 0x7F;
        if header_type == 0 {
            assert!(index < 6, "Invalid BAR index for header type 0: {}", index);
        } else if header_type == 1 {
            assert!(index < 2, "Invalid BAR index for header type 1: {}", index);
        } else {
            panic!("Header type {} does not conatin BARs", header_type);
        }
    }

    let first_bar = get_dword(0x10 + index * 4, dev);
    if first_bar == 0 {
        return None;
    }
    if first_bar & 0x1 == 0 {
        //memory space bar
        let physical_bar_addr: PhysAddr;
        let size: u64;
        let bars: u8;
        let prefetchable = (first_bar & 0b1000) != 0;
        let is_64_bit = (first_bar & 0b110) == 0b10;
        if is_64_bit {
            let second_bar = get_dword(0x10 + index * 4 + 4, dev);
            let address = (first_bar & 0xFFFF_FFF0) as u64 | ((second_bar as u64) << 32);
            physical_bar_addr = PhysAddr(address);
            bars = 2;
            size = get_64b_mem_bar_size(index, dev);
        } else {
            physical_bar_addr = PhysAddr(first_bar as u64 & 0xFFFF_FFF0);
            bars = 1;
            size = get_bar_size(index, 0xF, dev) as u64;
        }
        println!("BAR {}: addr={:#X}, size={:#X}", index, physical_bar_addr.0, size);

        let num = size.div_ceil(4096);
        if num > 256 {
            return None
        }
        Some((Bar::Memory(MemoryBar::new(index, 0x10 + index * 4, physical_bar_addr, size, prefetchable, is_64_bit)), bars))
    } else {
        //io space bar
        let address = first_bar as u16 & 0xFFFC;
        let size = get_bar_size(index, 0x3, dev);
        Some((Bar::IO(IOBar::new(index, address, size)), 1))
    }
}

fn get_bar_size(index: u8, mask: u32, dev: &PciDeviceLocation) -> u32 {
    let bar = get_dword(0x10 + index * 4, dev);
    set_dword(0x10 + index * 4, 0xFFFF_FFFF, dev);
    let size = get_dword(0x10 + index * 4, dev) & !mask;
    set_dword(0x10 + index * 4, bar, dev);
    (!size) + 1
}

fn get_64b_mem_bar_size(index: u8, dev: &PciDeviceLocation) -> u64 {
    let bar0 = get_dword(0x10 + index * 4, dev);
    let bar1 = get_dword(0x10 + (index + 1) * 4, dev);
    
    set_dword(0x10 + index * 4, 0xFFFF_FFFF, dev);
    set_dword(0x10 + (index + 1) * 4, 0xFFFF_FFFF, dev);

    let size0 = get_dword(0x10 + index * 4, dev);
    let size1 = get_dword(0x10 + (index + 1) * 4, dev);

    set_dword(0x10 + index * 4, bar0, dev);
    set_dword(0x10 + (index + 1) * 4, bar1, dev);

    !(((size1 as u64) << 32) | size0 as u64) + 1
}

pub fn get_capabilities_pointer(dev: &PciDeviceLocation) -> u8 {
    (get_dword(0x34, dev) & 0b11111100) as u8
}

pub fn load_capabilities_list(dev: &mut LegacyPciDevice) {
    let status = get_status(&dev.common.device);
    if (status & 0x10) == 0 {
        return;
    }
    let mut capabilities = Vec::new();
    let mut pointer = get_capabilities_pointer(&dev.common.device);
    while pointer != 0 {
        let capability_first_dword = get_dword(pointer, &dev.common.device);
        let capability_id = capability_first_dword as u8;
        capabilities.push(Capability {
            id: capability_id,
            pointer,
        });
        pointer = (capability_first_dword >> 8) as u8;
    }
    dev.common.capabilities = capabilities;
}
