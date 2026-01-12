// E1000e Network Device Driver
// Follows the 82574 specification


use std::{error::ErrorCode, mem_utils::VirtAddr};

use crate::drivers::pci::{self, BarTrait, NetworkController, PciClass, PciDeviceNumericId};

mod registers;

pub(super) fn init_driver() {
    crate::drivers::pci::add_pcie_driver((
        PciClass::NetworkController(NetworkController::EthernetController),
        PciDeviceNumericId {
            vendor_id: Some(0x8086), //Intel
            device_id: Some(0x10D3), //82574L
            subvendor_id: None, //Intel again, but there is no subdevice so don't care
            subdevice_id: None,
        }
    ), init_e1000e);
}

fn init_e1000e(dev: pci::PcieDevice) {
    panic!("E1000e driver not implemented yet");

}

struct E1000eDevice {
    device: pci::PcieDevice,
    memory_bar: VirtAddr,
    flash_bar: VirtAddr,
}

impl E1000eDevice {
    pub fn new(device: pci::PcieDevice) -> Result<Self, ErrorCode> {

        let mem_bar = device
            .bars
            .iter()
            .find(|bar| bar.get_index() == 0)
            .ok_or(ErrorCode::NoEntry)?;
        let flash_bar = device
            .bars
            .iter()
            .find(|bar| bar.get_index() == 1)
            .ok_or(ErrorCode::NoEntry)?;

        let memory_bar = mem_bar.get_address();
        let flash_bar = flash_bar.get_address();

        #[cfg(debug_assertions)]
        {
            use crate::drivers::net_device::e1000e::registers::E1000eRegistersPtr;

            let registers = unsafe { E1000eRegistersPtr::from_ptr(mem_bar.get_address().0 as *mut _) };
            let addr_0 = registers.as_ptr() as usize;
            let addr_last_field = registers.fcrtv().as_ptr() as usize;
            assert!(addr_last_field - addr_0 == 0x5F40); //when modifying regs, this needs to stay
                                                         //constant
        }

        Ok(Self {
            device,
            memory_bar,
            flash_bar,
        })
    }
}
