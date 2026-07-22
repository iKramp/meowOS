use crate::memory::{addresses::get_at_addr, current_root};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct VirtAddr(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct PhysAddr(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct PhysOffset(pub u64);

static mut HHDM_ADDR: PhysOffset = PhysOffset(0);
static mut HHDM_LEN: u64 = 0;

pub fn set_hhdm_addr(offset: PhysOffset) {
    unsafe { HHDM_ADDR = offset };
}
pub fn set_hhdm_len(len: u64) {
    unsafe { HHDM_LEN = len };
}
#[inline]
pub fn is_in_hhdm(addr: VirtAddr) -> bool {
    unsafe {
        let end = HHDM_ADDR.0 + HHDM_LEN;
        addr.0 >= HHDM_ADDR.0 && addr.0 < end
    }
}

#[inline]
pub fn is_phys_in_hhdm(addr: PhysAddr) -> bool {
    let virt = addr + unsafe { HHDM_ADDR };
    unsafe {
        let end = HHDM_ADDR.0 + HHDM_LEN;
        virt.0 >= HHDM_ADDR.0 && virt.0 < end
    }
}

impl core::ops::Add<PhysOffset> for PhysAddr {
    type Output = VirtAddr;
    #[inline]
    fn add(self, rhs: PhysOffset) -> Self::Output {
        VirtAddr(self.0 + rhs.0)
    }
}
impl core::ops::Sub<PhysOffset> for VirtAddr {
    type Output = PhysAddr;
    #[inline]
    fn sub(self, rhs: PhysOffset) -> Self::Output {
        PhysAddr(self.0 - rhs.0)
    }
}
impl core::convert::From<PhysAddr> for VirtAddr {
    #[inline]
    fn from(value: PhysAddr) -> Self {
        assert!(is_phys_in_hhdm(value), "phys addr {:#x} is not in HHDM", value.0);
        value + unsafe { HHDM_ADDR }
    }
}

pub fn translate_virt_phys_addr(addr: VirtAddr, root_page_addr: Option<PhysAddr>) -> Option<PhysAddr> {
    if is_in_hhdm(addr) {
        return Some(addr - unsafe { HHDM_ADDR });
    }

    let mut page_addr = root_page_addr.unwrap_or_else(current_root);
    #[allow(clippy::unusual_byte_groupings)] //they are grouped by section masks
    let mut final_mask: u64 = 0b111111111_111111111_111111111_111111111_111111111111;
    let mask = 0b111_111_111_000;
    for level in (1..5).rev() {
        let offset = PhysAddr((addr.0 >> (level * 9)) & mask);
        final_mask >>= 9;
        let page_entry = unsafe { *get_at_addr::<u64, _>(page_addr + offset) };
        let present = page_entry & 1 != 0;
        if !present {
            return None;
        }
        page_addr = PhysAddr(page_entry & 0xFFFFFFFFFF000);
        let huge_page = page_entry & 0b10000000 != 0;
        if huge_page {
            break;
        }
    }
    //here we have the page of the data
    Some(page_addr + PhysAddr(addr.0 & final_mask))
}
