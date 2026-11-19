use core::fmt::Debug;
use core::fmt::Display;

use bitfield::{Bit, BitMut, bitfield};

use crate::memory::{MemoryCachingStrategy, addresses::*};

bitfield! {
    pub struct PageTableEntry(u64);
    // impl Debug;
    pub present, set_present: 0;
    pub writeable, set_writeable: 1;
    pub user_accessible, set_user_accessible: 2;
    pub(in crate::arch::x86_64::memory) page_write_through, set_page_write_through: 3;
    pub(in crate::arch::x86_64::memory) page_cache_disable, set_page_cache_disable: 4;
    pub accessed, _: 5;
    pub dirty, _: 6;
    pub global, set_global: 8;
    pub(in crate::arch::x86_64::memory) reserved, _: 51, 48;
    pub(in crate::arch::x86_64::memory) no_execute, set_no_execute: 63;
}

impl Display for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Format the output
        f.debug_struct("PageTableEntry")
            .field("present", &self.present())
            .field("writeable", &self.writeable())
            .field("user accessible", &self.user_accessible())
            .field("write through", &self.page_write_through())
            .field("disable cache", &self.page_cache_disable())
            .field("accessed", &self.accessed())
            .field("dirty", &self.dirty())
            .field("global", &self.global())
            .field("reserved", &self.reserved())
            .field("no execute", &self.no_execute())
            .field("address", &self.address(1))
            .field("huge page", &self.bit(7)) //not real for all levels
            .finish()
    }
}

impl Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self, f)
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
    pub fn new(phys_address: PhysAddr, user_mode: bool, level: u8) -> Self {
        let mut entry = Self((phys_address.0 & 0x_FFF_FFF_FFF_000) | 0b000000011 | (1 << 63));
        if user_mode {
            entry.0 |= 4;
        }

        entry.set_caching_strategy(MemoryCachingStrategy::WriteBack, level);

        entry
    }

    pub fn blank() -> Self {
        Self(0)
    }

    pub fn address(&self, level: u8) -> PhysAddr {
        debug_assert!((1..=4).contains(&level));
        let huge = self.huge_page(level);
        PhysAddr(self.0 & 0xF_FFF_FFF_FFF_000)
    }

    pub fn set_address(&mut self, address: PhysAddr, level: u8, huge: bool) {
        const MASK: u64 = 0xF_FFF_FFF_FFF_000;
        self.0 = (self.0 & !MASK) | (address.0 & MASK);
    }

    fn set_pat(&mut self, pat: LiminePat, level: u8) {
        let (pat, pcd, pwt) = pat.to_pat_pcd_pwt();
        self.set_page_cache_disable(pcd);
        self.set_page_write_through(pwt);
        self.set_pat_bit(level, pat);
    }

    fn pat(&self, level: u8) -> LiminePat {
        let pcd = self.page_cache_disable();
        let pwt = self.page_write_through();
        let pat = self.pat_bit(level);

        LiminePat::from_pat_pcd_pwt(pat, pcd, pwt)
    }

    pub fn set_caching_strategy(&mut self, caching: MemoryCachingStrategy, level: u8) {
        self.set_pat(caching.into(), level);
    }

    pub fn caching_strategy(&self, level: u8) -> MemoryCachingStrategy {
        self.pat(level).into()
    }

    pub fn executable(&self) -> bool {
        !self.no_execute()
    }

    pub fn set_executable(&mut self, executable: bool) {
        self.set_no_execute(!executable);
    }

    fn get(tree_root: PhysAddr, entry_virt: VirtAddr) -> Option<&'static mut Self> {
        let mut page_node_addr = VirtAddr::from(tree_root);
        for level in (1..=4).rev() {
            let index = (entry_virt.0 >> (12 + (level - 1) * 9)) & 0b111111111;
            let page_table_entry = unsafe { &mut *(page_node_addr.0 as *mut Self).add(index as usize) };
            if !page_table_entry.present() {
                return None;
            }
            if page_table_entry.huge_page(level) || level == 1 {
                return Some(page_table_entry);
            }
            page_node_addr = page_table_entry.address(level).into();
        }
        unreachable!()
    }

    pub fn huge_page(&self, level: u8) -> bool {
        match level {
            2 => self.bit(7),
            3 => self.bit(7),
            _ => false,
        }
    }

    pub fn set_huge_page(&mut self, level: u8, huge: bool) {
        match level {
            2 => self.set_bit(7, huge),
            3 => self.set_bit(7, huge),
            _ => {}
        }
    }

    fn pat_bit(&self, level: u8) -> bool {
        match level {
            1 => self.bit(7),
            2 => self.bit(12),
            3 => self.bit(12),
            _ => true, //invalid
        }
    }

    fn set_pat_bit(&mut self, level: u8, pat: bool) {
        match level {
            1 => self.set_bit(7, pat),
            2 => self.set_bit(12, pat),
            3 => self.set_bit(12, pat),
            _ => {}
        }
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
