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
    pci::{self, BarTrait, NetworkController, PciClass, PcieDriver},
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
    crate::drivers::pci::register_express_pci_driver(
        PciClass::NetworkController(NetworkController::EthernetController),
        Some(0x8086), //Intel
        Some(0x10D3), //82574L
        None,
        None,
        || Box::new(E1000eDriver),
    );
}

struct E1000eDriver;

impl PcieDriver for E1000eDriver {
    fn init(&mut self, dev: &pci::PcieDevice) {
        init_e1000e(dev);
    }

    fn deinit(&mut self, dev: &pci::PcieDevice) {

    }

    fn remove_device(&mut self) {
        todo!()
    }

    fn service_interrupt(&mut self, dev: &pci::PcieDevice) {
        todo!()
    }
}

pub static mut E1000E_DEVICE: Option<E1000eDevice> = None;

fn init_e1000e(dev: &pci::PcieDevice) {
    println!("Initializing e1000e device");
    let Ok(mut e1000e_device) = E1000eDevice::new(dev) else {
        println!("e1000e device has incorrect bars");
        return;
    };
    init::init(&mut e1000e_device);
    unsafe {
        E1000E_DEVICE = Some(e1000e_device);
    }
}

struct E1000eDevice<'a> {
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
    pub fn new(device: &pci::PcieDevice) -> Result<Self, ErrorCode> {
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

    fn print_ptrs(&self) {
        let rx_head = self.registers.rx_descriptor_queue_info().rdh().read();
        let rx_tail = self.registers.rx_descriptor_queue_info().rdt().read();
        let tx_head = self.registers.tx_descriptor_queue_info().tdh().read();
        let tx_tail = self.registers.tx_descriptor_queue_info().tdt().read();
        println!("RX Head: {}, RX Tail: {}", rx_head, rx_tail);
        println!("TX Head: {}, TX Tail: {}", tx_head, tx_tail);

        //status registers
        let status = self.registers.icr().read();
        println!("interrupt Register: 0x{:X}", status.0);
    }
}

pub fn print_ptrs() {
    unsafe {
        if let Some(dev) = &E1000E_DEVICE {
            dev.print_ptrs();
        }
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
