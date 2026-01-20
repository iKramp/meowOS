use std::{
    boxed::Box,
    mem_utils::{PhysAddr, VirtAddr, translate_virt_phys_addr},
};

use crate::{
    drivers::net_device::e1000e::E1000eDevice,
    memory::{paging::PageTree, physical_allocator},
};

#[derive(Debug)]
#[repr(C)]
pub(super) struct TransmitDescriptor {
    pub buffer_addr: PhysAddr,
    pub length: u16,
    pub checksum_offset: u8,
    pub command: u8,
    pub status_extcmd: u8,
    pub checksum_start: u8,
    pub vlan: u16,
}

pub(super) const TX_DESC_COUNT: usize = 256;

pub(super) fn init_transmit(dev: &mut E1000eDevice) {
    dev.registers.tctl().write(
        *dev.registers
            .tctl()
            .read()
            .set_en(false) //disable transmit for now
            .set_ct(15) //collision threshold - ignored in full duplex
            .set_cold(0x3F)
            .set_swxoff(false)
            .set_pbe(false)
            .set_psp(true)
    );

    let queue_size_bytes = TX_DESC_COUNT * std::mem::size_of::<TransmitDescriptor>();
    let mut tx_queue: Box<[TransmitDescriptor; TX_DESC_COUNT]> = Box::new(std::array::from_fn(|_| TransmitDescriptor::default()));
    let tx_queue_virt = VirtAddr(tx_queue.as_ref() as *const _ as u64);
    let tx_queue_phys =
        translate_virt_phys_addr(tx_queue_virt, PageTree::get_level4_addr()).expect("Failed to translate TX queue address");

    for descriptor in tx_queue.iter_mut() {
        let phys_addr = physical_allocator::allocate_frame();
        descriptor.buffer_addr = phys_addr;
        descriptor.length = 0;
        descriptor.checksum_offset = 0;
        descriptor.command = 0;
        descriptor.status_extcmd = 0;
        descriptor.checksum_start = 0;
        descriptor.vlan = 0;
    }

    dev.transmit_queue = Some((tx_queue, (tx_queue_virt, tx_queue_phys)));
    dev.registers
        .tx_descriptor_queue_info()
        .tdbal()
        .write((tx_queue_phys.0 & 0xFFFF_FFFF) as u32);
    dev.registers
        .tx_descriptor_queue_info()
        .tdbah()
        .write((tx_queue_phys.0 >> 32) as u32);
    dev.registers
        .tx_descriptor_queue_info()
        .tdlen()
        .write((queue_size_bytes) as u32);
    dev.registers.tx_descriptor_queue_info().tdh().write(0);
    dev.registers.tx_descriptor_queue_info().tdt().write(0);
    dev.registers.tx_descriptor_queue_info().txdctl().write(
        *dev.registers.tx_descriptor_queue_info().txdctl().read()
            .set_gran(true)
            .set_wthresh(1)
            .set_lwthresh(0)
            .set_hthresh(0)
            .set_pthresh(0)
    );

    dev.registers.tipg().write(*dev.registers.tipg().read()
        .set_ipgt(8)
        .set_ipgr1(2)
        .set_ipgr2(10)
    );
}

pub fn enable_transmit(dev: &mut E1000eDevice) {
    let mut tctl = dev.registers.tctl().read();
    tctl.set_en(true);
    dev.registers.tctl().write(tctl);
}

pub fn disable_transmit(dev: &mut E1000eDevice) {
    let mut tctl = dev.registers.tctl().read();
    tctl.set_en(false);
    dev.registers.tctl().write(tctl);
}

impl Drop for TransmitDescriptor {
    fn drop(&mut self) {
        if self.buffer_addr.0 != 0 {
            unsafe { physical_allocator::deallocate_frame(self.buffer_addr) };
        }
    }
}

impl Default for TransmitDescriptor {
    fn default() -> Self {
        Self {
            buffer_addr: PhysAddr(0),
            length: 0,
            checksum_offset: 0,
            command: 0,
            status_extcmd: 0,
            checksum_start: 0,
            vlan: 0,
        }
    }
}

impl Clone for TransmitDescriptor {
    fn clone(&self) -> Self {
        if self.buffer_addr.0 == 0 {
            Self {
                buffer_addr: self.buffer_addr,
                length: self.length,
                checksum_offset: self.checksum_offset,
                command: self.command,
                status_extcmd: self.status_extcmd,
                checksum_start: self.checksum_start,
                vlan: self.vlan,
            }
        } else {
            //new frame, just copy data
            let new_phys_addr = physical_allocator::allocate_frame();
            Self {
                buffer_addr: new_phys_addr,
                length: self.length,
                checksum_offset: self.checksum_offset,
                command: self.command,
                status_extcmd: self.status_extcmd,
                checksum_start: self.checksum_start,
                vlan: self.vlan,
            }
        }
    }
}
