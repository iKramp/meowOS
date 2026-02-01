use std::mem_utils::*;

use crate::acpi::sdt::AcpiSdtHeader;

pub trait RootSystemDescriptorTable {
    fn validate(&self) -> bool {
        let addr = self.get_addr() as u64;
        let mut sum: u16 = 0;
        for i in 0..self.length() {
            unsafe {
                sum += *get_at_virtual_addr::<u8>(VirtAddr(addr + i as u64)) as u16;
            }
        }
        sum & 0xFF == 0
    }

    fn get_addr(&self) -> *const u8;
    fn length(&self) -> u32;
    fn get_table(&self, signature: [u8; 4]) -> Option<PhysAddr>;
    fn get_tables(&self) -> std::Vec<PhysAddr>;
    fn print_tables(&self);
    fn print_signature(&self);
}

pub fn get_rsdt(rsdp: &super::rsdp::Rsdp) -> &'static dyn RootSystemDescriptorTable {
    unsafe {
        let address = rsdp.address();
        let virt_address = translate_phys_virt_addr(address);

        let table_ptr = virt_address.0 as *const AcpiSdtHeader;
        let table_len = (table_ptr.byte_add(4) as *const u32).read_unaligned();
        let virt_address = std::mem_utils::ensure_aligned_manual(virt_address, table_len as u64, 8);

        #[cfg(debug_assertions)]
        match rsdp {
            super::rsdp::Rsdp::V1(_) => crate::println!("V1"),
            super::rsdp::Rsdp::V2(_) => crate::println!("V2"),
        }

        match rsdp {
            super::rsdp::Rsdp::V1(_) => &mut *(virt_address.0 as *mut Rsdt),
            super::rsdp::Rsdp::V2(_) => &mut *(virt_address.0 as *mut Xsdt),
        }
    }
}

#[repr(C)]
struct Rsdt {
    header: AcpiSdtHeader,
}

impl RootSystemDescriptorTable for Rsdt {
    fn get_addr(&self) -> *const u8 {
        self as *const Self as *const u8
    }

    fn length(&self) -> u32 {
        self.header.length
    }
    fn get_table(&self, signature: [u8; 4]) -> Option<PhysAddr> {
        unsafe {
            let start_table_ptr = VirtAddr((self.get_addr()) as u64 + 36);
            let num_entries = (self.length() - 36) / 4;
            for entry_index in 0..num_entries {
                let table_entry_ptr = start_table_ptr + (entry_index as u64 * 4);
                let table_ptr = PhysAddr(*get_at_virtual_addr::<u32>(table_entry_ptr) as u64);
                let header = get_at_physical_addr::<super::sdt::AcpiSdtHeader>(table_ptr);
                if header.signature == signature {
                    return Some(table_ptr);
                }
            }
            None
        }
    }

    fn get_tables(&self) -> std::Vec<PhysAddr> {
        let mut tables = std::Vec::new();
        unsafe {
            let start_table_ptr = VirtAddr((self.get_addr()) as u64 + 36);
            let num_entries = (self.length() - 36) / 4;
            for entry_index in 0..num_entries {
                let table_entry_ptr = start_table_ptr + (entry_index as u64 * 4);
                let table_ptr = PhysAddr(*get_at_virtual_addr::<u32>(table_entry_ptr) as u64);
                tables.push(table_ptr);
            }
            tables
        }
    }

    fn print_tables(&self) {
        unsafe {
            let start_table_ptr = VirtAddr((self as *const Self) as u64 + 36);
            let num_entries = (self.length() - 36) / 4;
            for entry_index in 0..num_entries {
                let table_entry_ptr = start_table_ptr + (entry_index as u64 * 4);
                let table_ptr = PhysAddr(*get_at_virtual_addr::<u32>(table_entry_ptr) as u64);
                let header = get_at_physical_addr::<super::sdt::AcpiSdtHeader>(table_ptr);
                crate::println!("{:?}", header.signature.map(|a| a as char))
            }
        }
    }
    fn print_signature(&self) {
        crate::println!(
            "{:?}",
            self.header.signature.iter().map(|a| *a as char).collect::<std::Vec<char>>()
        )
    }
}

#[repr(C)]
#[derive(Debug)]
struct Xsdt {
    header: AcpiSdtHeader,
}

impl RootSystemDescriptorTable for Xsdt {
    fn get_addr(&self) -> *const u8 {
        self as *const Self as *const u8
    }

    fn length(&self) -> u32 {
        self.header.length
    }

    fn get_table(&self, signature: [u8; 4]) -> Option<PhysAddr> {
        unsafe {
            let start_table_ptr = VirtAddr((self.get_addr()) as u64 + 36);
            let num_entries = (self.length() - 36) / 8;
            for entry_index in 0..num_entries {
                let table_entry_ptr = start_table_ptr + (entry_index as u64 * 8);
                let table_ptr = PhysAddr(*get_at_virtual_addr::<u64>(table_entry_ptr));
                let header = get_at_physical_addr::<super::sdt::AcpiSdtHeader>(table_ptr);
                if header.signature == signature {
                    return Some(table_ptr);
                }
            }
            None
        }
    }

    fn get_tables(&self) -> std::Vec<PhysAddr> {
        let mut tables = std::Vec::new();
        unsafe {
            let start_table_ptr = VirtAddr((self.get_addr()) as u64 + 36);
            let num_entries = (self.length() - 36) / 8;
            for entry_index in 0..num_entries {
                let table_entry_ptr = start_table_ptr + (entry_index as u64 * 8);
                let table_ptr = PhysAddr(*get_at_virtual_addr::<u64>(table_entry_ptr));
                tables.push(table_ptr);
            }
        }
        tables
    }

    fn print_tables(&self) {
        unsafe {
            let start_table_ptr = VirtAddr((self.get_addr()) as u64 + 36);
            let num_entries = (self.length() - 36) / 8;
            for entry_index in 0..num_entries {
                let table_entry_ptr = start_table_ptr + (entry_index as u64 * 8);
                let table_ptr = PhysAddr(*get_at_virtual_addr::<u64>(table_entry_ptr));
                let header = get_at_physical_addr::<super::sdt::AcpiSdtHeader>(table_ptr);
                crate::println!("{:?}", header.signature.map(|a| a as char))
            }
        }
    }

    fn print_signature(&self) {
        crate::println!(
            "{:?}",
            self.header.signature.iter().map(|a| *a as char).collect::<std::Vec<char>>()
        )
    }
}
