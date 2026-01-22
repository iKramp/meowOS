// E1000e Network Device Driver
// Follows the 82574 specification

use std::{
    boxed::Box,
    error::ErrorCode,
    mem_utils::{PhysAddr, VirtAddr},
    println,
};

use crate::drivers::{
    net_device::e1000e::{
        receive::{RX_DESC_COUNT, ReceiveDescriptor},
        registers::PhyAddress,
        transmit::{TX_DESC_COUNT, TransmitDescriptor},
    },
    pci::{self, BarTrait},
};

mod init;
mod mdio;
mod nvm;
mod phy;
mod receive;
mod registers;
mod statistics;
mod transmit;

pub(super) fn init_driver() {
    // crate::drivers::pci::add_pcie_driver(
    //     (
    //         PciClass::NetworkController(NetworkController::EthernetController),
    //         PciDeviceNumericId {
    //             vendor_id: Some(0x8086), //Intel
    //             device_id: Some(0x10D3), //82574L
    //             subvendor_id: None,      //Intel again, but there is no subdevice so don't care
    //             subdevice_id: None,
    //         },
    //     ),
    //     init_e1000e,
    // );
}

fn init_e1000e(dev: pci::PcieDevice) {
    let Ok(mut e1000e_device) = E1000eDevice::new(dev) else {
        println!("e1000e device has incorrect bars");
        return;
    };
    init::init(&mut e1000e_device);
    std::mem::forget(e1000e_device); //leak the device for now
}

struct E1000eDevice<'a> {
    device: pci::PcieDevice,
    memory_bar: VirtAddr,
    flash_bar: VirtAddr,
    registers: registers::E1000eRegistersPtr<'a>, //same value as memory_bar but typed
    phy_addr: PhyAddress,
    phy_id: u32,
    mac_address: [u8; 6],
    link_up: bool,
    receive_queue: Option<(Box<[ReceiveDescriptor; RX_DESC_COUNT]>, (VirtAddr, PhysAddr))>,
    transmit_queue: Option<(Box<[TransmitDescriptor; TX_DESC_COUNT]>, (VirtAddr, PhysAddr))>,
}

impl E1000eDevice<'_> {
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
        let registers = unsafe { registers::E1000eRegistersPtr::from_ptr(memory_bar.0 as *mut _) };

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
            registers,
            phy_addr: PhyAddress::ExternalGigabit,
            phy_id: 0,
            mac_address: [0; 6],
            link_up: false,
            receive_queue: None,
            transmit_queue: None,
        })
    }
}

fn enable_promiscuous_mode(dev: &mut E1000eDevice) {
    let mut rctl = dev.registers.rctl().read();
    rctl.set_upe(true).set_mpe(true);
    dev.registers.rctl().write(rctl);
}
fn disable_promiscuous_mode(dev: &mut E1000eDevice) {
    let mut rctl = dev.registers.rctl().read();
    rctl.set_upe(false).set_mpe(false);
    dev.registers.rctl().write(rctl);
}
