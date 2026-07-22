use std::{println, vec::Vec};

use crate::memory::addresses::{OwnedPhysAddr, PhysAddr};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct OwnedPhysRange(pub PhysRange);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct PhysRange {
    pub start: PhysAddr,
    pub n_pages: u64,
}

pub trait OwnedPhysicalRangeData {
    fn get_range(&self) -> PhysRange;
}

impl OwnedPhysicalRangeData for OwnedPhysRange {
    fn get_range(&self) -> PhysRange {
        self.0
    }
}

impl Drop for OwnedPhysRange {
    fn drop(&mut self) {
        if self.0.start.0 == 0 {
            println!(level:warn, "Dropping an OwnedPhysRange with start address 0, this is likely a bug. Find cause");
            return;
        }
        unsafe { crate::memory::physical_allocator::_deallocate_by_ref(self) };
    }
}

impl core::convert::From<OwnedPhysAddr> for OwnedPhysRange {
    fn from(addr: OwnedPhysAddr) -> Self {
        let range = Self(PhysRange {
            start: addr.0,
            n_pages: 1,
        });
        core::mem::forget(addr); //don't deallocate
        range
    }
}

impl OwnedPhysRange {
    pub fn empty() -> Self {
        Self(PhysRange {
            start: PhysAddr(0),
            n_pages: 0,
        })
    }
    pub fn shrink_to(&mut self, new_range: PhysRange) {
        let old_range = self.0;
        assert!(new_range.start >= self.0.start);
        assert!(new_range.start + new_range.n_pages * 4096 <= self.0.start + self.0.n_pages * 4096);
        self.0 = new_range;

        let range_before = PhysRange {
            start: self.0.start,
            n_pages: (new_range.start - self.0.start).0 / 4096,
        };
        let range_after = PhysRange {
            start: new_range.start + new_range.n_pages * 4096,
            n_pages: old_range.n_pages - new_range.n_pages - range_before.n_pages,
        };

        let range_before = OwnedPhysRange(range_before);
        let range_after = OwnedPhysRange(range_after);
        drop(range_before);
        drop(range_after);
    }
    pub fn break_into_individual(self) -> Vec<OwnedPhysAddr> {
        let new_vec = self.0.get_addresses().map(OwnedPhysAddr).collect::<Vec<_>>();
        core::mem::forget(self); //don't deallocate
        new_vec
    }
}

impl PhysRange {
    pub fn get_addresses(&self) -> impl Iterator<Item = PhysAddr> {
        let start = self.start;
        let end = self.start + self.n_pages * 4096;
        (start.0..end.0).step_by(4096).map(PhysAddr)
    }
}
