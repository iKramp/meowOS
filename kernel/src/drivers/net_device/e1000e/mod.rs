// E1000e Network Device Driver
// Follows the 82574 specification

use core::mem::MaybeUninit;
use std::{
    boxed::Box,
    error::ErrorCode,
    mem_utils::{PhysAddr, VirtAddr},
    println, r_lock_w_info,
    sync::{no_int_spinlock::NoIntSpinlock, rw_lock::RWSpinlock},
    w_lock_w_info,
};

use crate::{
    drivers::{
        net_device::e1000e::{
            receive::{RX_DESC_COUNT, ReceiveDescriptor},
            registers::PhyAddress,
            transmit::{TX_DESC_COUNT, TransmitDescriptor},
        },
        pci::{self, BarTrait, NetworkController, PciClass, PcieDriver},
    },
    memory::paging::PageTree,
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
        || {
            Box::new(E1000eDriver {
                device: MaybeUninit::uninit(),
            })
        },
    );
}

struct E1000eDriver {
    device: MaybeUninit<E1000eDevice>,
}

impl PcieDriver for E1000eDriver {
    fn init(&mut self, dev: &pci::PcieDevice) -> Result<(), ErrorCode> {
        let e1000edev = init_e1000e(dev)?;
        self.device = MaybeUninit::new(e1000edev);
        Ok(())
    }

    fn deinit(&mut self, _dev: &pci::PcieDevice) {}

    fn remove_device(&mut self) {
        todo!()
    }

    fn service_interrupt(&mut self, _dev: &pci::PcieDevice) {
        let nic = unsafe { self.device.assume_init_mut() };
        nic.service_interrupt();
    }
}

fn init_e1000e(dev: &pci::PcieDevice) -> Result<E1000eDevice, ErrorCode> {
    println!("Initializing e1000e device");
    let Ok(mut e1000e_device) = E1000eDevice::new(dev) else {
        println!("e1000e device has incorrect bars");
        return Err(ErrorCode::IllegalValue);
    };
    dev.enable_bus_mastering();
    init::init(&mut e1000e_device);
    Ok(e1000e_device)
}

struct E1000eDevice {
    memory_bar: VirtAddr,
    flash_bar: VirtAddr,
    registers: RWSpinlock<registers::E1000eRegistersPtr<'static>>, //same value as memory_bar but typed
    phy_addr: PhyAddress,
    phy_id: u32,
    mac_address: [u8; 6],
    link_up: bool,
    #[allow(clippy::type_complexity)] //not even that complex
    receive_queue: Option<(&'static mut [ReceiveDescriptor; RX_DESC_COUNT], (VirtAddr, PhysAddr))>,
    #[allow(clippy::type_complexity)] //not even that complex
    transmit_queue: Option<(&'static mut [TransmitDescriptor; TX_DESC_COUNT], (VirtAddr, PhysAddr))>,
    receive_lock: NoIntSpinlock<()>,
    transmit_lock: NoIntSpinlock<()>,
}

impl E1000eDevice {
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
            registers: RWSpinlock::new(registers),
            phy_addr: PhyAddress::ExternalGigabit,
            phy_id: 0,
            mac_address: [0; 6],
            link_up: false,
            receive_queue: None,
            transmit_queue: None,
            receive_lock: NoIntSpinlock::new(()),
            transmit_lock: NoIntSpinlock::new(()),
        })
    }

    fn print_info(&self) {
        let registers = r_lock_w_info!(self.registers);
        let rx_head = registers.rx_descriptor_queue_info().rdh().read();
        let rx_tail = registers.rx_descriptor_queue_info().rdt().read();
        let tx_head = registers.tx_descriptor_queue_info().tdh().read();
        let tx_tail = registers.tx_descriptor_queue_info().tdt().read();
        println!("RX Head: {}, RX Tail: {}", rx_head, rx_tail);
        println!("TX Head: {}, TX Tail: {}", tx_head, tx_tail);

        //status registers
        let int_status = registers.icr().read();
        println!("interrupt Register: 0x{:X?}", int_status);
        let status = registers.status().read();
        println!("Status Register: 0x{:X?}", status);

        // dump registers
        println!(
            "RDBAL={:#x} RDBAH={:#x} RDLEN={:#x} RDH={} RDT={}",
            registers.rx_descriptor_queue_info().rdbal().read(),
            registers.rx_descriptor_queue_info().rdbah().read(),
            registers.rx_descriptor_queue_info().rdlen().read(),
            registers.rx_descriptor_queue_info().rdh().read(),
            registers.rx_descriptor_queue_info().rdt().read(),
        );

        println!(
            "TDBAL={:#x} TDBAH={:#x} TDLEN={:#x} TDH={} TDT={}",
            registers.tx_descriptor_queue_info().tdbal().read(),
            registers.tx_descriptor_queue_info().tdbah().read(),
            registers.tx_descriptor_queue_info().tdlen().read(),
            registers.tx_descriptor_queue_info().tdh().read(),
            registers.tx_descriptor_queue_info().tdt().read(),
        );

        println!("RCTL={:#x?}", registers.rctl().read(),);

        println!("TCTL={:#x?}", registers.tctl().read(),);
    }

    fn enable_promiscuous_mode(&mut self) {
        let registers = w_lock_w_info!(self.registers);
        let mut rctl = registers.rctl().read();
        rctl.set_upe(true).set_mpe(true);
        registers.rctl().write(rctl);
    }
    fn disable_promiscuous_mode(&mut self) {
        let registers = w_lock_w_info!(self.registers);
        let mut rctl = registers.rctl().read();
        rctl.set_upe(false).set_mpe(false);
        registers.rctl().write(rctl);
    }

    fn service_interrupt(&mut self) {
        let registers = r_lock_w_info!(self.registers);
        let icr = registers.icr().read();
        //we really care only about lsc, rxt0, rxo, rxdmt0, rxq0
        if icr.LSC() {
            self.link_up = registers.status().read().lu()
        }

        drop(registers);

        //RXO should have additional checks maybe
        if icr.RXDMT0() || icr.RXT0() || icr.RxQ0() || icr.RXO() {
            //process packets
            receive::process_received_packets(self);
        }
    }
}

impl Drop for E1000eDevice {
    fn drop(&mut self) {
        if let Some(queue) = &self.receive_queue {
            let queue_size_bytes = RX_DESC_COUNT * core::mem::size_of::<ReceiveDescriptor>();
            let queue_size_pages = queue_size_bytes.div_ceil(4096);

            let mut page_tree = PageTree::current();
            for page in 0..queue_size_pages {
                let res = page_tree.unmap(queue.1.0 + 4096 * page as u64);
                #[cfg(debug_assertions)]
                {
                    if let Err(e) = res {
                        println!("Error unmapping RX page {}: {:?}", page, e);
                    }
                }
            }
        }

        if let Some(queue) = &self.transmit_queue {
            let queue_size_bytes = TX_DESC_COUNT * core::mem::size_of::<TransmitDescriptor>();
            let queue_size_pages = queue_size_bytes.div_ceil(4096);

            let mut page_tree = PageTree::current();
            for page in 0..queue_size_pages {
                let res = page_tree.unmap(queue.1.0 + 4096 * page as u64);
                #[cfg(debug_assertions)]
                {
                    if let Err(e) = res {
                        println!("Error unmapping TX page {}: {:?}", page, e);
                    }
                }
            }
        }
    }
}
