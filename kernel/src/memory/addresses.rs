use core::{alloc::Layout, arch::asm};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct VirtAddr(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct PhysAddr(pub u64);
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PhysOffset(pub u64);

pub struct OwnedRange(pub MemoryRange);
pub struct MemoryRange {
    pub start: VirtAddr,
    pub n_pages: u64,
}

pub struct OwnedPhysRange(pub PhysRange);
pub struct PhysRange {
    pub start: PhysAddr,
    pub n_pages: u64,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct OwnedVirtAddr(pub VirtAddr);
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct OwnedPhysAddr(pub PhysAddr);

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
        assert!(value.0 < unsafe { HHDM_LEN });
        value + unsafe { HHDM_ADDR }
    }
}

macro_rules! impl_addr_ops {
    ($addr_type:ident) => {
        impl<T: Into<u64>> core::ops::Add<T> for $addr_type {
            type Output = $addr_type;
            #[inline]
            fn add(self, rhs: T) -> Self::Output {
                $addr_type(self.0 + rhs.into())
            }
        }
        impl<T: Into<u64>> core::ops::Sub<T> for $addr_type {
            type Output = $addr_type;
            #[inline]
            fn sub(self, rhs: T) -> Self::Output {
                $addr_type(self.0 - rhs.into())
            }
        }
        impl<T: Into<u64>> core::ops::AddAssign<T> for $addr_type {
            #[inline]
            fn add_assign(&mut self, rhs: T) {
                self.0 += rhs.into();
            }
        }
        impl<T: Into<u64>> core::ops::SubAssign<T> for $addr_type {
            #[inline]
            fn sub_assign(&mut self, rhs: T) {
                self.0 -= rhs.into();
            }
        }
        impl core::convert::From<$addr_type> for u64 {
            #[inline]
            fn from(addr: $addr_type) -> u64 {
                addr.0.into()
            }
        }
    };
}

impl core::convert::From<VirtAddr> for MemoryRange {
    #[inline]
    fn from(addr: VirtAddr) -> MemoryRange {
        MemoryRange { start: addr, n_pages: 1 }
    }
}
impl core::convert::From<PhysAddr> for MemoryRange {
    #[inline]
    fn from(addr: PhysAddr) -> MemoryRange {
        MemoryRange {
            start: addr.into(),
            n_pages: 1,
        }
    }
}
impl core::convert::From<OwnedVirtAddr> for MemoryRange {
    #[inline]
    fn from(addr: OwnedVirtAddr) -> MemoryRange {
        MemoryRange {
            start: addr.0,
            n_pages: 1,
        }
    }
}
impl core::convert::From<OwnedPhysAddr> for MemoryRange {
    #[inline]
    fn from(addr: OwnedPhysAddr) -> MemoryRange {
        MemoryRange {
            start: addr.0.into(),
            n_pages: 1,
        }
    }
}

impl_addr_ops!(VirtAddr);
impl_addr_ops!(PhysAddr);
impl_addr_ops!(OwnedVirtAddr);
impl_addr_ops!(OwnedPhysAddr);

///# Safety
///the address must be valid and there are no other references to the data
///This can be used as is if all the data has been written before using the function,
///becasue rust cannot rearrange memory reads when it comes to pointers (which are used)
///If data changes after this function is called, a read_volatile needs to be used
#[inline]
pub unsafe fn get_at_addr<T, P: Into<MemoryRange>>(addr: P) -> &'static mut T {
    let range: MemoryRange = addr.into();
    assert!(core::mem::size_of::<T>() <= range.n_pages as usize * 4096);
    let ptr = range.start.0 as *mut () as *mut T;
    unsafe { &mut *ptr }
}

///# Safety
///must be a valid addr (with no other data there)
#[inline]
pub unsafe fn set_at_addr<T, P: Into<MemoryRange>>(addr: P, data: T) {
    let range: MemoryRange = addr.into();
    assert!(core::mem::size_of::<T>() <= range.n_pages as usize * 4096);
    let ptr = range.start.0 as *mut () as *mut T;
    unsafe { ptr.write(data) };
}

///# Safety
///the virtual address offset must be correct
#[inline]
pub unsafe fn memset_at_addr<P: Into<MemoryRange>>(addr: P, value: u8, size: usize) {
    let range: MemoryRange = addr.into();
    assert!(range.n_pages * 4096 >= size as u64);
    unsafe {
        asm!(
            "rep stosb",
            in("rdi") range.start.0 as *mut u8,
            in("rcx") size,
            in("al") value,
            options(nostack)
        );
    }
}

///# Safety
///Can only be called on in-memory read only STATIC data structures
pub unsafe fn align<T>(addr: VirtAddr) -> VirtAddr {
    let align = core::mem::align_of::<T>() as u64;
    let size = core::mem::size_of::<T>() as u64;
    unsafe { align_manual(addr, size, align) }
}
///# Safety
///Can only be called on in-memory read only STATIC data structures
pub unsafe fn align_manual(addr: VirtAddr, size: u64, align: u64) -> VirtAddr {
    if addr.0.is_multiple_of(align) {
        addr
    } else {
        let heap_data = unsafe { std::alloc::alloc::alloc(Layout::from_size_align(size as usize, align as usize).unwrap()) };
        unsafe {
            core::ptr::copy_nonoverlapping(addr.0 as *const u8, heap_data, size as usize);
        }
        VirtAddr(heap_data as u64)
    }
}

pub fn translate_virt_phys_addr(addr: VirtAddr, root_page_addr: Option<PhysAddr>) -> Option<PhysAddr> {
    if is_in_hhdm(addr) {
        return Some(addr - unsafe { HHDM_ADDR });
    }

    let mut page_addr = root_page_addr?;
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

#[inline]
pub fn is_userspace_ptr(addr: VirtAddr) -> bool {
    addr.0 < 0x0000_8000_0000_0000
}

#[inline]
pub fn extend_addr(addr: VirtAddr) -> VirtAddr {
    if addr.0 & (1 << 47) != 0 {
        VirtAddr(addr.0 | 0xFFFF000000000000)
    } else {
        addr
    }
}

#[inline]
pub fn trim_addr_extension(addr: VirtAddr) -> VirtAddr {
    VirtAddr(addr.0 & 0x0000FFFFFFFFFFFF)
}

#[inline]
///# Safety
///Caller must ensure the lifetimes will work out, even though it may be impossible in rust's type
///system
pub unsafe fn set_static_lifetime<T>(data: &T) -> &'static T {
    let data_ptr = data as *const T;
    let static_data: &'static T = unsafe { &*data_ptr };
    static_data
}

#[inline]
///# Safety
///Caller must ensure the lifetimes will work out, even though it may be impossible in rust's type
///system
pub unsafe fn set_static_lifetime_mut<T>(data: &mut T) -> &'static mut T {
    let data_ptr = data as *mut T;
    let static_data: &'static mut T = unsafe { &mut *data_ptr };
    static_data
}
