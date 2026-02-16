use core::{cell::UnsafeCell, sync::atomic::Ordering};
use std::{
    lock_w_info,
    mem_utils::{PhysAddr, translate_virt_phys_addr},
    println,
    vec::Vec,
    w_lock_w_info,
};

use crate::{
    drivers::net_device::e1000e::E1000eDevice,
    memory::{
        paging::{self, PageTree},
        physical_allocator,
    },
    net,
};

#[derive(Debug)]
#[repr(C)]
pub(super) struct TransmitDescriptor {
    pub buffer_addr: PhysAddr,
    pub length: u16,
    pub checksum_offset: u8,
    pub command: TxDescCommand,
    pub status_extcmd: u8,
    pub checksum_start: u8,
    pub vlan: u16,
}

bitfield::bitfield! {
    pub(super) struct TxDescCommand(u8);
    impl Debug;
    interrupt_delay_enable, _: 7;
    vlan_enable, _: 6;
    descriptor_extension, _: 5;
    report_status, _: 3;
    insert_checksum, _: 2;
    insert_fcs, set_insert_fcs: 1;
    end_of_packet, set_eop: 0;
}

pub(super) const TX_DESC_COUNT: usize = 256;

pub(super) fn init_transmit(dev: &mut E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    registers.tctl().write(
        *registers
            .tctl()
            .read()
            .set_en(false) //disable transmit for now
            .set_ct(15) //collision threshold - ignored in full duplex
            .set_cold(0x3F)
            .set_swxoff(false)
            .set_pbe(false)
            .set_psp(true), //pad short packets
    );

    let queue_size_bytes = TX_DESC_COUNT * std::mem::size_of::<TransmitDescriptor>();
    let queue_size_pages = queue_size_bytes.div_ceil(4096);

    let tx_queue_virt = paging::PageTree::current().allocate_contigious(queue_size_pages as u64, None, false);
    let tx_queue: &mut [UnsafeCell<TransmitDescriptor>; TX_DESC_COUNT] =
        unsafe { &mut *(tx_queue_virt.0 as *mut [UnsafeCell<TransmitDescriptor>; TX_DESC_COUNT]) };

    let mut page_tree = paging::PageTree::current();
    for i in 0..queue_size_pages {
        let page_virt = tx_queue_virt + (i * 4096) as u64;
        let page = page_tree.get_page_table_entry_mut(page_virt).expect("was just allocated");
        page.set_pat(paging::LiminePat::UC);
    }

    let tx_queue_phys =
        translate_virt_phys_addr(tx_queue_virt, PageTree::get_level4_addr()).expect("Failed to translate TX queue address");

    for descriptor in tx_queue.iter_mut() {
        let phys_addr = physical_allocator::allocate_frame();
        let desc = unsafe { &mut *descriptor.get() };
        desc.buffer_addr = phys_addr;
        desc.length = 0;
        desc.checksum_offset = 0;
        desc.command = TxDescCommand(0);
        desc.status_extcmd = 0;
        desc.checksum_start = 0;
        desc.vlan = 0;
    }

    dev.transmit_queue = Some((tx_queue, (tx_queue_virt, tx_queue_phys)));
    registers
        .tx_descriptor_queue_info()
        .tdbal()
        .write((tx_queue_phys.0 & 0xFFFF_FFFF) as u32);
    registers
        .tx_descriptor_queue_info()
        .tdbah()
        .write((tx_queue_phys.0 >> 32) as u32);
    registers.tx_descriptor_queue_info().tdlen().write((queue_size_bytes) as u32);
    registers.tx_descriptor_queue_info().tdh().write(0);
    registers.tx_descriptor_queue_info().tdt().write(0);
    registers.tx_descriptor_queue_info().txdctl().write(
        *registers
            .tx_descriptor_queue_info()
            .txdctl()
            .read()
            .set_gran(true)
            .set_wthresh(1)
            .set_lwthresh(0)
            .set_hthresh(0)
            .set_pthresh(0),
    );

    registers
        .tipg()
        .write(*registers.tipg().read().set_ipgt(8).set_ipgr1(2).set_ipgr2(10));
}

pub(super) fn send_packet(dev: &E1000eDevice, packet: net::NetPacketListNode) {
    let raw_chunks = packet.into_raw_data();
    let mut tx_descriptors = Vec::new();
    for chunk in raw_chunks {
        let mut command = TxDescCommand(0);
        command.set_insert_fcs(true);

        let descriptor = TransmitDescriptor {
            buffer_addr: chunk.phys_addr(),
            length: chunk.len() as u16,
            command,
            ..TransmitDescriptor::default()
        };
        tx_descriptors.push(descriptor);
    }

    let Some(chunk) = tx_descriptors.last_mut() else {
        return;
    };
    chunk.command.set_eop(true);

    println!("sending packet with {} chunks", tx_descriptors.len());

    core::sync::atomic::fence(Ordering::Release);

    let lock = lock_w_info!(dev.transmit_lock);
    let registers = w_lock_w_info!(dev.registers);

    let tail = registers.tx_descriptor_queue_info().tdt().read() as usize;
    let head = registers.tx_descriptor_queue_info().tdh().read() as usize + TX_DESC_COUNT - 1; //to avoid negative, but can't use last

    println!("Transmit queue head: {}, tail: {}", head % TX_DESC_COUNT, tail);

    let available_slots = head - tail;
    if available_slots < tx_descriptors.len() {
        println!(level:error, "Not enough space in transmit queue: available {}, needed {}", available_slots, tx_descriptors.len());
        return;
    }

    let desc_len = tx_descriptors.len();
    for (i, descriptor) in tx_descriptors.into_iter().enumerate() {
        let index = (tail + i) % TX_DESC_COUNT;
        let Some((queue, _)) = dev.transmit_queue.as_ref() else {
            println!(level:error, "Transmit queue not initialized");
            return;
        };
        //safe because we hold the lock
        unsafe { *queue[index].get() = descriptor };
    }

    let new_tail = (tail + desc_len) % TX_DESC_COUNT;
    println!("New transmit queue tail: {}", new_tail);

    registers
        .tx_descriptor_queue_info()
        .tdt()
        .write(new_tail as u32);
    drop(lock);
}

pub fn enable_transmit(dev: &E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    let mut tctl = registers.tctl().read();
    tctl.set_en(true);
    registers.tctl().write(tctl);
}

pub fn disable_transmit(dev: &E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    let mut tctl = registers.tctl().read();
    tctl.set_en(false);
    registers.tctl().write(tctl);
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
            command: TxDescCommand(0),
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
