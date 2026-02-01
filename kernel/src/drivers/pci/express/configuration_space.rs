use std::{mem_utils::PhysAddr, print, println, vec::Vec};

use bitfield::bitfield;
use reg_map::RegMap;

use crate::{
    drivers::pci::{
        MemoryBar,
        capabilities::{Capability, CapabilityPtr, ExtendedCapability, ExtendedCapabilityPtr},
        express::express_device::PcieDevice,
    },
};

#[derive(Debug, RegMap)]
#[repr(C)]
pub(in crate::drivers::pci) struct LegacyConfigSpaceT0 {
    vendor_id: u16,
    device_id: u16,
    command: CommandReg,
    status: StatusReg,
    revision_id: u8,
    prog_if: u8,
    subclass: u8,
    class_code: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: HeaderType,
    bist: u8,
    bars: [u32; 6],
    cardbus_cis_pointer: u32,
    subsystem_vendor_id: u16,
    subsystem_id: u16,
    expansion_rom_addr: u32,
    cap_pointer: u8,
    type_specific_1: [u8; 7],
    interrupt_line: u8,
    interrupt_pin: u8,
    min_gnt: u8,
    max_lat: u8,
}

// Safety: No sync so must be behind a lock when used across threads
unsafe impl Send for LegacyConfigSpaceT0Ptr<'_> {}

pub fn get_capabilities_list(dev: &PcieDevice) -> Vec<Capability> {
    let mut capabilities = Vec::new();
    let mut pointer = dev.config_space_addr.cap_pointer().read();
    while pointer != 0 {
        let cap_ptr = dev.config_space_addr.as_ptr() as u64 + pointer as u64;
        let cap = unsafe { CapabilityPtr::from_ptr(cap_ptr as *mut Capability) };
        let capability = Capability {
            id: cap.id().read(),
            pointer,
        };
        capabilities.push(capability);
        pointer = cap.pointer().read();
    }

    capabilities
}

pub fn get_extended_capabilities_list(dev: &PcieDevice) -> Vec<ExtendedCapability> {
    let pointer = dev.config_space_addr.as_ptr() as u64 + 0x100;
    let mut ext_capabilities = Vec::new();
    let mut cap = unsafe { ExtendedCapabilityPtr::from_ptr(pointer as *mut ExtendedCapability) };
    println!("initial pointer is {:p}", cap.as_ptr());
    if cap.id().read() == 0 {
        return ext_capabilities;
    }
    loop {
        println!("reading pointer {:p}", cap.as_ptr());
        let cap_read = ExtendedCapability {
            id: cap.id().read(),
            version_and_pointer: cap.version_and_pointer().read(),
        };
        println!("{:?}", cap_read);
        ext_capabilities.push(cap_read);
        if cap_read.version_and_pointer.next_offset() == 0 {
            println!("no next cap");
            return ext_capabilities;
        } else {
            cap = unsafe {
                ExtendedCapabilityPtr::from_ptr(
                    (dev.config_space_addr.as_ptr() as u64 + cap_read.version_and_pointer.next_offset() as u64) as *mut ExtendedCapability,
                )
            }
        }
    }
}

pub fn get_bar(dev: &PcieDevice, index: u8) -> Option<MemoryBar> {

    #[cfg(debug_assertions)]
    {
        let header_type = dev.config_space_addr.header_type().read().header_type();
        if header_type == 0 {
            assert!(index < 6, "Invalid BAR index for header type 0: {}", index);
        } else if header_type == 1 {
            assert!(index < 2, "Invalid BAR index for header type 1: {}", index);
        } else {
            panic!("Header type {} does not conatin BARs", header_type);
        }
    }

    if index >= 6 {
        return None; //device doesn't have this bar
    }

    let curr_bar = unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).read() };
    let curr_bar_2 = if index < 5 { 
        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize + 1).read() }
    } else {
        0
    };

    if curr_bar & 1 == 1 {
        return None; //IO bar, not PCIe
    }

    let is_64_bit = (curr_bar >> 1) & 0b11 == 0b10;

    let prefetchable = (curr_bar >> 3) & 1 == 1;
    let (mut size, addr) = if is_64_bit {
        println!("bar is 64 bit");
        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).write(u32::MAX) };
        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize + 1).write(u32::MAX) };

        let bottom_bits = unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).read() };
        let top_bits = unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize + 1).read() };

        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).write(curr_bar) };
        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize + 1).write(curr_bar_2) };

        let size = ((top_bits as u64) << 32) | (bottom_bits as u64);
        let addr = ((curr_bar_2 as u64) << 32) | (curr_bar as u64 & !0b1111);
        (size, addr)
    } else {
        println!("bar is 32 bit");
        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).write(u32::MAX) };

        let bottom_bits = unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).read() };

        unsafe { dev.config_space_addr.bars().idx_unchecked(index as usize).write(curr_bar) };

        let size = bottom_bits as u64 | 0xFFFFFFFF00000000;
        let addr = curr_bar as u64 & !0b1111;
        (size, addr)
    };

    size &= !0b1111;
    size = !size;
    size += 1;
    println!("Size of bar: {:X}, address of bar: {:X}", size, addr);
    println!("first bar reg: {:X}", curr_bar);

    Some(MemoryBar::new(index, index + 0x10, PhysAddr(addr), size, prefetchable, is_64_bit))
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::pci) struct CommandReg(u16);
    impl Debug;
    pub io_space_enable, set_io_space_enable: 0;
    pub memory_space_enable, set_memory_space_enable: 1;
    pub bus_master_enable, set_bus_master_enable: 2;
    pub parity_error_response, set_parity_error_response: 6;
    pub serr_enable, set_serr_enable: 8;
    pub interrupt_disable, set_interrupt_disable: 10;
}

bitfield! {
    #[derive(RegMap)]
    struct StatusReg(u16);
    impl Debug;
    immediate_readiness, _: 0;
    interrupt_status, _: 3;
    master_data_pairity_error, clear_master_data_pairity_error: 8;
    signaled_target_abort, clear_signaled_target_abort: 11;
    received_target_abort, clear_received_target_abort: 12;
    received_master_abort, clear_received_master_abort: 13;
    signaled_system_error, clear_signaled_system_error: 14;
    detected_parity_error, clear_detected_parity_error: 15;
}

bitfield! {
    #[derive(RegMap)]
    pub(super) struct HeaderType(u8);
    impl Debug;
    pub multi_function, _: 7;
    pub header_type, _: 6, 0;
}
