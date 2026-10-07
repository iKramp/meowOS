use crate::memory::addresses::{PhysAddr, VirtAddr};

pub mod page_table;
pub mod page_table_entry;

pub const PAGE_SIZE_BITS: u8 = 12;
pub const PAGE_SIZE_BYTES: u32 = 1 << PAGE_SIZE_BITS as u32;

pub const PAGE_LEVEL_BITS: u8 = 9;
pub const PAGE_TABLE_ENTRIES: u32 = 1 << PAGE_LEVEL_BITS as u32;
pub const PAGE_TABLE_SIZE_BYTES: u32 = PAGE_SIZE_BYTES;

pub const PAGING_LEVELS: u8 = 4;

pub const VIRTUAL_ADDRESS_BITS: u8 = PAGE_SIZE_BITS + (PAGING_LEVELS * PAGE_LEVEL_BITS);
pub const PHYSICAL_ADDRESS_BITS: u8 = 52;

pub const PAGE_OFFSET_MASK: u64 = (1 << PAGE_SIZE_BITS) - 1;

pub fn set_page_tree_root(root: PhysAddr) {
    unsafe {
        core::arch::asm!(
            "mov cr3, {}",
            in(reg) root.0
        );
    }
}

pub fn current_root() -> PhysAddr {
    let mut level_4_table = PhysAddr(0);
    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) level_4_table.0,
        );
    }
    level_4_table
}

pub fn flush_tlb(addr: Option<VirtAddr>) {
    match addr {
        Some(addr) => unsafe {
            core::arch::asm!(
                "invlpg [{}]",
                in(reg) addr.0
            )
        },
        None => {
            set_page_tree_root(current_root());
        }
    }
}
