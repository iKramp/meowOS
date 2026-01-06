use std::mem_utils::VirtAddr;

use reg_map::RegMap;
use bitfield::bitfield;

use crate::drivers::pci::port_access::{get_dword_from_addr, set_dword_at_addr};

pub mod msi;
pub mod msix;

#[derive(Debug, Clone, Copy, RegMap)]
#[repr(C)]
pub(super) struct Capability {
    pub id: u8,
    pub pointer: u8,
}

#[derive(Debug, Clone, Copy, RegMap)]
#[repr(C)]
pub(super) struct ExtendedCapability {
    pub id: u16,
    pub version_and_pointer: ExtendedCapVersionPointer,
}

bitfield! {
    #[derive(RegMap, Clone, Copy)]
    pub(super) struct ExtendedCapVersionPointer(u16);
    impl Debug;
    pub cap_version, _: 3, 0;
    pub next_offset, _: 15, 4;
}

enum CapAddr {
    IO(u32),
    Memory(VirtAddr),
}

impl CapAddr {
    fn get_dword(&self, offset: u8) -> u32 {
        match self {
            CapAddr::IO(addr) => get_dword_from_addr(addr + offset as u32),
            CapAddr::Memory(addr) => unsafe { ((addr.0 + offset as u64) as *mut u32).read_volatile() }
        }
    }

    fn set_dword(&self, offset: u8, data: u32) {
        match self {
            CapAddr::IO(addr) => set_dword_at_addr(addr + offset as u32, data),
            CapAddr::Memory(addr) => unsafe { ((addr.0 + offset as u64) as *mut u32).write_volatile(data) }
        }
    }
}

