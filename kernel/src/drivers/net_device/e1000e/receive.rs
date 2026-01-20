use crate::{
    drivers::net_device::e1000e::{E1000eDevice, registers::MRQC},
    memory::{paging::PageTree, physical_allocator},
    rand,
};
use std::{
    boxed::Box,
    mem_utils::{PhysAddr, VirtAddr, translate_virt_phys_addr},
    println,
};

pub(super) const RX_DESC_COUNT: usize = 256;

#[repr(C)]
pub(super) struct ReceiveDescriptor {
    pub buffer_addr: PhysAddr,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub vlan_tag: u16,
}

pub(super) fn init_receive(dev: &mut E1000eDevice) {
    //mac addr 0
    let mac_reg = dev.registers.rx_add().idx(0);
    let mac = mac_reg.ral().read() as u64 | ((mac_reg.rah().read() as u64) << 32);
    println!("MAC address read from hardware: {:012X}", mac);
    if mac == 0 {
        let random_mac = generate_random_mac();
        println!(
            "No MAC address found in hardware, generating random MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            random_mac[0], random_mac[1], random_mac[2], random_mac[3], random_mac[4], random_mac[5],
        );
        mac_reg.ral().write(
            (random_mac[0] as u32)
                | ((random_mac[1] as u32) << 8)
                | ((random_mac[2] as u32) << 16)
                | ((random_mac[3] as u32) << 24),
        );
        mac_reg.rah().write(
            (random_mac[4] as u32) | ((random_mac[5] as u32) << 8) | 0x80000000, //set valid bit
        );
        dev.mac_address = random_mac;
    } else {
        //valid mac
        dev.mac_address = [
            (mac & 0xFF) as u8,
            ((mac >> 8) & 0xFF) as u8,
            ((mac >> 16) & 0xFF) as u8,
            ((mac >> 24) & 0xFF) as u8,
            ((mac >> 32) & 0xFF) as u8,
            ((mac >> 40) & 0xFF) as u8,
        ];
    }

    //zero out MTA
    for i in 0..128 {
        dev.registers.mta().idx(i).write(0);
    }

    dev.registers.rctl().write(
        *dev.registers
            .rctl()
            .read()
            .set_en(false) //disable first
            .set_sbp(false) //no store bad packets
            .set_upe(false) //no promiscuous
            .set_mpe(false) //no promiscuous
            .set_lpe(false) //no long packets for now
            .set_lbm(0) //no loopback
            .set_rdmts(0) //process at 1/2 descriptors filled
            .set_dtyp(0) //split descriptor
            //ignore multicast offset for now, idk what it is
            .set_bam(true) //broadcast accept mode
            .set_bsize(0b11) //4096 bytes buffer size - 1 page
            .set_bsex(true) //buffer size extension
            .set_vfe(false) //no vlan filtering
            .set_cfien(false)
            .set_dpf(true) //discard pause frames - not dealing with pause now
            .set_pmcf(true) //pass MAC control frames
            .set_secrc(false), //don't strip crc
    );

    dev.registers.mrqc().write(MRQC(0)); //no multi-queue

    //initialize the queue
    let queue_size_bytes = RX_DESC_COUNT * core::mem::size_of::<ReceiveDescriptor>();
    let mut rx_queue: Box<[ReceiveDescriptor; RX_DESC_COUNT]> = Box::new(std::array::from_fn(|_| ReceiveDescriptor::default()));
    let rx_queue_virt = VirtAddr(rx_queue.as_ref() as *const _ as u64);
    let rx_queue_phy =
        translate_virt_phys_addr(rx_queue_virt, PageTree::get_level4_addr()).expect("Failed to translate RX queue address");

    for descriptor in rx_queue.iter_mut() {
        let phys_addr = physical_allocator::allocate_frame();
        descriptor.buffer_addr = phys_addr;
        descriptor.status = 0;
        descriptor.length = 0;
        descriptor.checksum = 0;
        descriptor.errors = 0;
        descriptor.vlan_tag = 0;
    }

    dev.receive_queue = Some((rx_queue, (rx_queue_virt, rx_queue_phy)));

    dev.registers.rx_descriptor_queue_info().rdbal().write(rx_queue_phy.0 as u32);
    dev.registers.rx_descriptor_queue_info().rdbah().write((rx_queue_phy.0 >> 32) as u32);
    dev.registers.rx_descriptor_queue_info().rdlen().write(queue_size_bytes as u32);
    dev.registers.rx_descriptor_queue_info().rdh().write(0);
    dev.registers.rx_descriptor_queue_info().rdt().write((RX_DESC_COUNT - 1) as u32);
}

pub fn enable_receive(dev: &mut E1000eDevice) {
    let mut rctl = dev.registers.rctl().read();
    rctl.set_en(true);
    dev.registers.rctl().write(rctl);
}

pub fn disable_receive(dev: &mut E1000eDevice) {
    let mut rctl = dev.registers.rctl().read();
    rctl.set_en(false);
    dev.registers.rctl().write(rctl);
}

pub fn generate_random_mac() -> [u8; 6] {
    [
        rand::rand_u8() | 0x02, //locally administered
        rand::rand_u8(),
        rand::rand_u8(),
        rand::rand_u8(),
        rand::rand_u8(),
        rand::rand_u8(),
    ]
}

impl Default for ReceiveDescriptor {
    fn default() -> Self {
        Self {
            buffer_addr: PhysAddr(0),
            length: 0,
            checksum: 0,
            status: 0,
            errors: 0,
            vlan_tag: 0,
        }
    }
}

impl Drop for ReceiveDescriptor {
    fn drop(&mut self) {
        if self.buffer_addr.0 != 0 {
            unsafe { physical_allocator::deallocate_frame(self.buffer_addr) };
        }
    }
}

impl Clone for ReceiveDescriptor {
    fn clone(&self) -> Self {
        if self.buffer_addr.0 == 0 {
            Self {
                buffer_addr: self.buffer_addr,
                length: self.length,
                checksum: self.checksum,
                status: self.status,
                errors: self.errors,
                vlan_tag: self.vlan_tag,
            }
        } else {
            //new frame, just copy the data
            let new_phys_addr = physical_allocator::allocate_frame();
            Self {
                buffer_addr: new_phys_addr,
                length: self.length,
                checksum: self.checksum,
                status: self.status,
                errors: self.errors,
                vlan_tag: self.vlan_tag,
            }
        }
    }
}
