use crate::{
    drivers::pci::PciDeviceLocation,
    utils::{dword_from_port, dword_to_port},
};

const CONFIG_ADDRESS: u16 = 0x0CF8;
const CONFIG_DATA: u16 = 0x0CFC;

pub fn get_dword(offset: u8, dev: &PciDeviceLocation) -> u32 {
    let config_address = get_config_address(true, dev.bus, dev.device, dev.function, offset);
    get_dword_from_addr(config_address)
}

pub fn get_dword_from_addr(conf_addr: u32) -> u32 {
    dword_to_port(CONFIG_ADDRESS, conf_addr);
    dword_from_port(CONFIG_DATA)
}

pub fn set_dword(offset: u8, value: u32, dev: &PciDeviceLocation) {
    let config_address = get_config_address(true, dev.bus, dev.device, dev.function, offset);
    set_dword_at_addr(config_address, value);
}

pub fn set_dword_at_addr(conf_addr: u32, value: u32) {
    dword_to_port(CONFIG_ADDRESS, conf_addr);
    dword_to_port(CONFIG_DATA, value);
}

pub(super) fn get_config_address(enable: bool, bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    debug_assert!(offset & 0b11 == 0);
    debug_assert!(function < 8);
    debug_assert!(device < 32);
    (if enable { 1 } else { 0 } << 31)
        | (bus as u32) << 16
        | ((device & 0x1F) as u32) << 11
        | ((function & 0b111) as u32) << 8
        | (offset as u32)
}
