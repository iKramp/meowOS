use core::fmt::Display;

use bitfield::bitfield;

use crate::memory::{MemoryCachingStrategy, addresses::*};

bitfield! {
    pub struct PageTableEntry(u64);
    impl Debug;
    pub present, set_present: 0;
    pub writeable, set_writeable: 1;
    pub user_accessible, set_user_accessible: 2;
    pub(in crate::arch::x86_64::memory) page_write_through, set_page_write_through: 3;
    pub(in crate::arch::x86_64::memory) page_cache_disable, set_page_cache_disable: 4;
    pub accessed, _: 5;
    pub dirty, _: 6;
    pub huge_page, set_huge_page: 7; //is shared with pat
    pub global, set_global: 8;
    pub(in crate::arch::x86_64::memory) reserved, _: 51, 48;
    pub(in crate::arch::x86_64::memory) no_execute, set_no_execute: 63;
}

pub fn entry_caching_strategy(entry: &PageTableEntry) -> MemoryCachingStrategy {
    let pat = false;
    let pcd = entry.page_cache_disable();
    let pwt = entry.page_write_through();

    LiminePat::from_pat_pcd_pwt(pat, pcd, pwt).into()
}

impl Display for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Format the output
        f.debug_struct("PageTableEntry")
            .field("present", &self.present())
            .field("address", &self.address())
            .field("huge page", &self.huge_page())
            .field("no execute", &self.no_execute())
            .field("writeable", &self.writeable())
            .field("write through", &self.page_write_through())
            .field("disable cache", &self.page_cache_disable())
            .field("user accessible", &self.user_accessible())
            .field("accessed", &self.accessed())
            .field("dirty", &self.dirty())
            .field("global", &self.global())
            .finish()
    }
}

//first 4 are identical as at power-on/reset
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiminePat {
    WB = 0,
    WT = 1,
    UCMinus = 2,
    UC = 3,
    WP = 4,
    WC = 5,
}

impl LiminePat {
    fn from_pat_pcd_pwt(pat: bool, pcd: bool, pwt: bool) -> Self {
        match (pat, pcd, pwt) {
            (false, false, false) => LiminePat::WB,
            (false, false, true) => LiminePat::WT,
            (false, true, false) => LiminePat::UCMinus,
            (false, true, true) => LiminePat::UC,
            (true, false, false) => LiminePat::WP,
            (true, false, true) => LiminePat::WC,
            (true, true, false) => LiminePat::UC, //invalid
            (true, true, true) => LiminePat::UC,  //invalid
        }
    }

    fn to_pat_pcd_pwt(&self) -> (bool, bool, bool) {
        match self {
            LiminePat::WB => (false, false, false),
            LiminePat::WT => (false, false, true),
            LiminePat::UCMinus => (false, true, false),
            LiminePat::UC => (false, true, true),
            LiminePat::WP => (true, false, false),
            LiminePat::WC => (true, false, true),
        }
    }
}

impl PageTableEntry {
    //creates default entry:
    //present, writeable, not write-through, not cache disabled, not accessed,
    //not dirty, not huge, not global, no execute, user/supervisor based on argument
    pub fn new(phys_address: PhysAddr, user_mode: bool) -> Self {
        let mut entry = Self((phys_address.0 & 0x_FFF_FFF_FFF_000) | 0b000000011 | (1 << 63));
        if user_mode {
            entry.0 |= 4;
        }

        entry.set_pat(LiminePat::WB);

        entry
    }

    pub fn blank() -> Self {
        Self(0)
    }

    pub fn address(&self) -> PhysAddr {
        PhysAddr(self.0 & 0xF_FFF_FFF_FFF_000)
    }

    pub fn set_address(&mut self, address: PhysAddr) {
        const MASK: u64 = 0xF_FFF_FFF_FFF_000;
        self.0 = (self.0 & !MASK) | (address.0 & MASK);
    }

    pub(in crate::arch::x86_64::memory) fn set_pat(&mut self, pat: LiminePat) {
        let (_pat, pcd, pwt) = pat.to_pat_pcd_pwt();
        self.set_page_cache_disable(pcd);
        self.set_page_write_through(pwt);
        //for now i ignore pat teehee :3
        //pat bit depends on if it's a page directory or page table. Can be checked with huge
        //table, but huge-huge tables (1GB) also have huge tables, and don't have pat bit
    }

    pub(in crate::arch::x86_64::memory) fn pat(&self) -> LiminePat {
        let pcd = self.page_cache_disable();
        let pwt = self.page_write_through();

        LiminePat::from_pat_pcd_pwt(false, pcd, pwt)
    }

    pub fn set_caching_strategy(&mut self, caching: MemoryCachingStrategy) {
        self.set_pat(caching.into());
    }

    pub fn caching_strategy(&self) -> MemoryCachingStrategy {
        entry_caching_strategy(self)
    }

    pub fn executable(&self) -> bool {
        !self.no_execute()
    }

    pub fn set_executable(&mut self, executable: bool) {
        self.set_no_execute(!executable);
    }

    fn get(tree_root: PhysAddr, entry_virt: VirtAddr) -> Option<&'static mut Self> {
        let mut page_node_addr = VirtAddr::from(tree_root);
        for level in (0..4).rev() {
            let index = (entry_virt.0 >> (12 + level * 9)) & 0b111111111;
            let page_table_entry = unsafe { &mut *(page_node_addr.0 as *mut Self).add(index as usize) };
            if !page_table_entry.present() {
                return None;
            }
            if page_table_entry.huge_page() || level == 0 {
                return Some(page_table_entry);
            }
            page_node_addr = page_table_entry.address().into();
        }
        unreachable!()
    }
}

impl From<MemoryCachingStrategy> for LiminePat {
    fn from(strategy: MemoryCachingStrategy) -> Self {
        match strategy {
            MemoryCachingStrategy::WriteBack => LiminePat::WB,
            MemoryCachingStrategy::WriteThrough => LiminePat::WT,
            MemoryCachingStrategy::Uncacheable => LiminePat::UC,
            MemoryCachingStrategy::WriteProtected => LiminePat::WP,
            MemoryCachingStrategy::WriteCombining => LiminePat::WC,
        }
    }
}

impl From<LiminePat> for MemoryCachingStrategy {
    fn from(pat: LiminePat) -> Self {
        match pat {
            LiminePat::WB => MemoryCachingStrategy::WriteBack,
            LiminePat::WT => MemoryCachingStrategy::WriteThrough,
            LiminePat::UCMinus => MemoryCachingStrategy::Uncacheable,
            LiminePat::UC => MemoryCachingStrategy::Uncacheable,
            LiminePat::WP => MemoryCachingStrategy::WriteProtected,
            LiminePat::WC => MemoryCachingStrategy::WriteCombining,
        }
    }
}
