use std::{
    mem_utils::{PhysAddr, VirtAddr},
    sync::once_lock::OnceLock,
    vec::Vec,
};

use crate::memory;

pub trait BarTrait {
    fn write_to_bar<T: Sized>(&self, data: &T, offset: u64);
    fn read_from_bar<T: Sized>(&self, offset: u64) -> T;
    fn get_index(&self) -> u8;
}

#[derive(Debug)]
pub enum Bar {
    Memory(MemoryBar),
    IO(IOBar),
}

#[derive(Debug)]
pub struct MemoryBar {
    pub index: u8,
    pub offset_in_conf_space: u8,
    phys_address: PhysAddr,
    prefetchable: bool,
    address: OnceLock<VirtAddr>,
    pub size: u64,
    pub is_64_bit: bool,
}

#[derive(Debug, Clone)]
pub struct IOBar {
    index: u8,
    address: u16,
    size: u32,
}

impl Bar {
    pub fn write_to_bar<T>(&self, data: &T, offset: u64) {
        match self {
            Bar::Memory(mem_bar) => {
                mem_bar.write_to_bar(data, offset);
            }
            Bar::IO(io_bar) => {
                io_bar.write_to_bar(data, offset);
            }
        }
    }

    pub fn read_from_bar<T: Sized>(&mut self, offset: u64) -> T {
        match self {
            Bar::Memory(mem_bar) => mem_bar.read_from_bar(offset),
            Bar::IO(io_bar) => io_bar.read_from_bar(offset),
        }
    }

    pub fn get_index(&self) -> u8 {
        match self {
            Bar::Memory(mem_bar) => mem_bar.get_index(),
            Bar::IO(io_bar) => io_bar.get_index(),
        }
    }
}

impl MemoryBar {
    pub fn new(index: u8, offset_in_conf_space: u8, address: PhysAddr, size: u64, prefetchable: bool, is_64_bit: bool) -> Self {
        Self {
            index,
            address: OnceLock::new(),
            phys_address: address,
            prefetchable,
            size,
            offset_in_conf_space,
            is_64_bit,
        }
    }

    pub fn get_address(&self) -> VirtAddr {
        *self.address.get_or_init(|| self.map(self.phys_address, self.prefetchable))
    }
}

impl BarTrait for MemoryBar {
    fn write_to_bar<T: Sized>(&self, data: &T, offset: u64) {
        let address = (self.get_address().0 + offset) as *mut T;
        assert!(
            offset + core::mem::size_of::<T>() as u64 <= self.size,
            "Data exceeds BAR size"
        );
        unsafe {
            core::ptr::copy_nonoverlapping(core::ptr::from_ref(data), address, 1);
        }
    }

    fn read_from_bar<T: Sized>(&self, offset: u64) -> T {
        let address = (self.get_address().0 + offset) as *const T;
        assert!(
            offset + core::mem::size_of::<T>() as u64 <= self.size,
            "Data exceeds BAR size"
        );
        unsafe { core::ptr::read_volatile(address) }
    }

    fn get_index(&self) -> u8 {
        self.index
    }
}

impl IOBar {
    pub fn new(index: u8, address: u16, limit: u32) -> Self {
        Self {
            index,
            address,
            size: limit,
        }
    }
}

impl BarTrait for IOBar {
    fn write_to_bar<T: Sized>(&self, data: &T, offset: u64) {
        let data = unsafe { core::slice::from_raw_parts(data as *const T as *const u8, core::mem::size_of::<T>()) };
        let address = self.address + offset as u16;
        assert!(offset + data.len() as u64 <= self.size as u64, "Data exceeds BAR size");
        for i in 0..data.len() {
            crate::utils::byte_to_port(address + i as u16, data[i]);
        }
    }

    fn read_from_bar<T: Sized>(&self, offset: u64) -> T {
        let mut data = Vec::with_capacity(core::mem::size_of::<T>());
        let address = self.address + offset as u16;
        assert!(offset + data.len() as u64 <= self.size as u64, "Data exceeds BAR size");
        for i in 0..core::mem::size_of::<T>() {
            data.push(crate::utils::byte_from_port(address + i as u16));
        }
        unsafe { core::ptr::read(data.as_ptr() as *const T) }
    }

    fn get_index(&self) -> u8 {
        self.index
    }
}

impl Drop for MemoryBar {
    fn drop(&mut self) {
        let Some(&addr) = self.address.get() else {
            return;
        };
        let pages = self.size.div_ceil(0x1000);
        unsafe { memory::kernel_manual_unmap(addr, pages, None) };
    }
}

impl MemoryBar {
    fn map(&self, phys_addr: PhysAddr, prefetchable: bool) -> VirtAddr {
        let pages = self.size.div_ceil(0x1000);

        let page_tree_root = memory::current_root();

        let (virt_addr, _entry) = unsafe { memory::kernel_manual_map(phys_addr, pages, Some(page_tree_root)) };
        for i in 0..pages {
            let page_entry = memory::get_page_table_entry(virt_addr + i * 0x1000, Some(page_tree_root)).expect("just allocated");
            if prefetchable {
                page_entry.set_pat(memory::LiminePat::WT, virt_addr + i * 0x1000);
            } else {
                page_entry.set_pat(memory::LiminePat::UC, virt_addr + i * 0x1000);
            }
        }
        virt_addr
    }
}
