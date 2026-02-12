use crate::{
    drivers::net_device::e1000e::{E1000eDevice, registers::MRQC},
    memory::{
        paging::{self, PageTree},
        physical_allocator,
    },
    net::{NetPacket, NetPacketSource, RawNetDataChunk},
    rand,
};
use bitfield::bitfield;
use core::sync::atomic::Ordering;
use std::{
    lock_w_info,
    mem_utils::{PhysAddr, translate_virt_phys_addr},
    println,
    vec::Vec,
    w_lock_w_info,
};

pub(super) const RX_DESC_COUNT: usize = 256;

#[repr(C)]
pub(super) struct ReceiveDescriptor {
    pub buffer_addr: PhysAddr,
    pub length: u16,
    pub checksum: u16,
    pub status: RxDescStatus,
    pub errors: u8,
    pub vlan_tag: u16,
}

bitfield! {
    pub(super) struct RxDescStatus(u8);
    impl Debug;
    desc_done, _: 0;
    end_of_packet, _: 1;
    vp, _: 3;
    udp_checksum_calculated, _: 4;
    tcp_checksum_calculated, _: 5;
    ip_checksum_calculated, _: 6;
}

pub(super) fn init_receive(dev: &mut E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    //mac addr 0
    let mac_reg = registers.rx_add().idx(0);
    let mac = mac_reg.ral().read() as u64 | ((mac_reg.rah().read() as u64) << 32);
    println!("MAC address read from hardware: {:012X}", mac);
    if mac & (1 << 63) == 0 {
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
        registers.mta().idx(i).write(0);
    }

    registers.rctl().write(
        *registers
            .rctl()
            .read()
            .set_en(false) //disable first
            .set_sbp(false) //no store bad packets
            .set_upe(false) //no promiscuous
            .set_mpe(false) //no promiscuous
            .set_lpe(false) //no long packets for now
            .set_lbm(0) //no loopback
            .set_rdmts(0) //process at 1/2 descriptors filled
            .set_dtyp(0) //legacy descriptor
            //ignore multicast offset for now, idk what it is
            .set_bam(true) //broadcast accept mode
            .set_bsize(0b11) //4096 bytes buffer size - 1 page
            .set_bsex(true) //buffer size extension
            .set_vfe(false) //no vlan filtering
            .set_cfien(false)
            .set_dpf(true) //discard pause frames - not dealing with pause now
            .set_pmcf(true) //pass MAC control frames
            .set_secrc(true), //strip crc
    );

    registers.mrqc().write(MRQC(0)); //no multi-queue

    //clear RSS redirection table
    for i in 0..32 {
        registers.reta().idx(i).write(0);
    }

    //clear RSS hash keys
    for i in 0..10 {
        registers.rssrk().idx(i).write(0);
    }

    //initialize the queue
    let queue_size_bytes = RX_DESC_COUNT * core::mem::size_of::<ReceiveDescriptor>();
    let queue_size_pages = queue_size_bytes.div_ceil(4096);

    let rx_queue_virt = paging::PageTree::current().allocate_contigious(queue_size_pages as u64, None, false);
    let rx_queue: &mut [ReceiveDescriptor; RX_DESC_COUNT] =
        unsafe { &mut *(rx_queue_virt.0 as *mut [ReceiveDescriptor; RX_DESC_COUNT]) };

    let mut page_tree = PageTree::current();
    for page in 0..queue_size_pages {
        let page_virt = rx_queue_virt + 4096 * page as u64;
        let page = page_tree.get_page_table_entry_mut(page_virt).expect("was just allocated");
        page.set_pat(paging::LiminePat::UC);
    }

    let rx_queue_phys =
        translate_virt_phys_addr(rx_queue_virt, PageTree::get_level4_addr()).expect("Failed to translate RX queue address");

    for descriptor in rx_queue[..RX_DESC_COUNT - 1].iter_mut() {
        let mut default_desc = ReceiveDescriptor::default();
        core::mem::swap(&mut default_desc, descriptor);
        core::mem::forget(default_desc);

        let phys_addr = physical_allocator::allocate_frame();
        descriptor.buffer_addr = phys_addr;
    }
    let last_descriptor = rx_queue.last_mut().expect("?");
    let mut default_desc = ReceiveDescriptor::default();
    core::mem::swap(&mut default_desc, last_descriptor);
    core::mem::forget(default_desc);

    core::sync::atomic::fence(Ordering::SeqCst);
    dev.receive_queue = Some((rx_queue, (rx_queue_virt, rx_queue_phys)));

    registers.rx_descriptor_queue_info().rdbal().write(rx_queue_phys.0 as u32);
    registers
        .rx_descriptor_queue_info()
        .rdbah()
        .write((rx_queue_phys.0 >> 32) as u32);
    registers.rx_descriptor_queue_info().rdlen().write(queue_size_bytes as u32);
    registers.rx_descriptor_queue_info().rdh().write(0);
    registers.rx_descriptor_queue_info().rdt().write((RX_DESC_COUNT - 1) as u32);

    registers.rxdctl().write(
        *registers
            .rxdctl()
            .read()
            .set_gran(true)
            .set_pthresh(32)
            .set_hthresh(32)
            .set_wthresh(1),
    );
}

pub(super) fn process_received_packets(dev: &mut E1000eDevice) {
    let packets = get_received_packets(dev);
    let net_packets = packets
        .into_iter()
        .map(|packet| {
            let data_chunk = RawNetDataChunk::new(packet.buffer_addr, packet.length.into());
            //each is a separate packet for now
            core::mem::forget(packet);
            NetPacket::from_single(data_chunk, crate::net::NetLayerType::Ethernet, NetPacketSource::Nic(dev.identifier))
        })
        .collect::<Vec<NetPacket>>();
    for packet in net_packets {
        crate::net::debug_packet(packet);
    }
}

fn get_received_packets(dev: &mut E1000eDevice) -> Vec<ReceiveDescriptor> {
    let lock = lock_w_info!(dev.receive_lock);
    let registers = w_lock_w_info!(dev.registers);
    let mut curr_tail = registers.rx_descriptor_queue_info().rdt().read();
    let curr_head = registers.rx_descriptor_queue_info().rdh().read();

    let Some((receive_queue, _)) = &mut dev.receive_queue else {
        return Vec::new();
    };

    let mut desc_vec = Vec::new();

    //first descriptor may have already been processed but can't be advanced. Check
    loop {
        let curr_descriptor = &mut receive_queue[curr_tail as usize];
        if curr_descriptor.buffer_addr.0 != 0 {
            //process descriptor
            let mut tmp_desc = ReceiveDescriptor::default();
            core::mem::swap(&mut tmp_desc, curr_descriptor);
            desc_vec.push(tmp_desc);
        }

        if (curr_tail + 1) % RX_DESC_COUNT as u32 == curr_head {
            break;
        }

        let data_frame = physical_allocator::allocate_frame();
        curr_descriptor.buffer_addr = data_frame;
        curr_tail += 1;
        curr_tail %= RX_DESC_COUNT as u32;
    }

    registers.rx_descriptor_queue_info().rdt().write(curr_tail);

    drop(lock);
    desc_vec
}

pub fn enable_receive(dev: &mut E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    let mut rctl = registers.rctl().read();
    rctl.set_en(true);
    registers.rctl().write(rctl);
}

pub fn disable_receive(dev: &mut E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    let mut rctl = registers.rctl().read();
    rctl.set_en(false);
    registers.rctl().write(rctl);
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
            status: RxDescStatus(0),
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
