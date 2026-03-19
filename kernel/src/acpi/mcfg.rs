use std::{boxed::Box, mem_utils::PhysAddr, println, vec::Vec};

#[derive(Debug)]
#[repr(C)]
pub struct McfgTable {
    header: crate::acpi::sdt::AcpiSdtHeader,
    reserved: u32,
    reserved2: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct BaseAddressAllocation {
    base_addr_low: u32, //split because it may be unaligned
    base_addr_high: u32,
    pci_segment_group_num: u16,
    start_bus_num: u8,
    end_bus_num: u8,
    reserved: u32,
}

impl McfgTable {
    pub fn allocations(&self) -> Box<[&'static BaseAddressAllocation]> {
        println!("McfgTable::allocations: finding allocations for table at {:p}", self);
        let mut entries = Vec::new();
        for i in 0..self.num_allocations() {
            let ptr = unsafe {
                (self as *const McfgTable as *mut McfgTable)
                    .byte_add(core::mem::size_of::<McfgTable>() + i * core::mem::size_of::<BaseAddressAllocation>())
                    as *mut BaseAddressAllocation
            };
            println!("McfgTable::allocations: found allocation at {:p}", ptr);
            entries.push(unsafe { &*ptr });
        }

        entries.into_boxed_slice()
    }

    pub fn num_allocations(&self) -> usize {
        let total_size = self.header.length as usize;
        let base_size = std::mem::size_of::<McfgTable>();
        let entry_size = std::mem::size_of::<BaseAddressAllocation>();
        (total_size - base_size) / entry_size
    }
}

impl BaseAddressAllocation {
    pub fn base_address(&self) -> PhysAddr {
        PhysAddr(((self.base_addr_high as u64) << 32) | (self.base_addr_low as u64))
    }

    pub fn pci_segment_group_number(&self) -> u16 {
        self.pci_segment_group_num
    }

    pub fn start_bus_number(&self) -> u8 {
        self.start_bus_num
    }

    pub fn end_bus_number(&self) -> u8 {
        self.end_bus_num
    }
}
