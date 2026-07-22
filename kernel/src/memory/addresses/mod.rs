use core::{alloc::Layout, arch::asm};

mod phys_range;
mod simple_address;

pub use phys_range::{OwnedPhysRange, OwnedPhysicalRangeData, PhysRange};
pub use simple_address::*;

use crate::memory::kernel_manual_unmap;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct OwnedVirtRange(pub VirtRange);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct VirtRange {
    pub start: VirtAddr,
    pub n_pages: u64,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
///This type holds as much ownership as possible:
///When in HHDM, it owns the physical address, but not translation (HHDM is always mapped)
///When in MMIO, it owns the translation, but not the physical address (MMIO cannot be allocated/freed)
///Else, it owns both the physical address and translation (normal memory)
///
///It will always try to free physical memory, but that will be a noop for memory not on ram
pub struct OwnedVirtAddr(pub VirtAddr);
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct OwnedPhysAddr(pub PhysAddr);

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

impl core::convert::From<VirtAddr> for VirtRange {
    #[inline]
    fn from(addr: VirtAddr) -> VirtRange {
        VirtRange { start: addr, n_pages: 1 }
    }
}
impl core::convert::From<PhysAddr> for VirtRange {
    #[inline]
    fn from(addr: PhysAddr) -> VirtRange {
        VirtRange {
            start: addr.into(),
            n_pages: 1,
        }
    }
}
impl core::convert::From<&OwnedVirtAddr> for VirtRange {
    #[inline]
    fn from(addr: &OwnedVirtAddr) -> VirtRange {
        VirtRange {
            start: addr.0,
            n_pages: 1,
        }
    }
}
impl core::convert::From<&OwnedPhysAddr> for VirtRange {
    #[inline]
    fn from(addr: &OwnedPhysAddr) -> VirtRange {
        VirtRange {
            start: addr.0.into(),
            n_pages: 1,
        }
    }
}
impl core::convert::From<&OwnedPhysRange> for VirtRange {
    #[inline]
    fn from(range: &OwnedPhysRange) -> VirtRange {
        VirtRange {
            start: range.0.start.into(),
            n_pages: range.0.n_pages,
        }
    }
}

impl core::convert::From<&OwnedPhysAddr> for VirtAddr {
    #[inline]
    fn from(addr: &OwnedPhysAddr) -> VirtAddr {
        addr.0.into()
    }
}

impl core::convert::From<OwnedPhysAddr> for OwnedVirtAddr {
    #[inline]
    fn from(addr: OwnedPhysAddr) -> OwnedVirtAddr {
        let phys_addr = addr.0;
        let virt_addr = phys_addr.into();
        core::mem::forget(addr);
        OwnedVirtAddr(virt_addr)
    }
}

impl core::convert::From<OwnedPhysRange> for OwnedVirtRange {
    #[inline]
    fn from(range: OwnedPhysRange) -> OwnedVirtRange {
        let phys_range = range.0;
        let virt_range = VirtRange {
            start: phys_range.start.into(),
            n_pages: phys_range.n_pages,
        };
        core::mem::forget(range);
        OwnedVirtRange(virt_range)
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
pub unsafe fn get_at_addr<T, P: Into<VirtRange>>(addr: P) -> &'static mut T {
    let range: VirtRange = addr.into();
    assert!(core::mem::size_of::<T>() <= range.n_pages as usize * 4096);
    let ptr = range.start.0 as *mut () as *mut T;
    unsafe { &mut *ptr }
}

///# Safety
///must be a valid addr (with no other data there)
#[inline]
pub unsafe fn set_at_addr<T, P: Into<VirtRange>>(addr: P, data: T) {
    let range: VirtRange = addr.into();
    assert!(core::mem::size_of::<T>() <= range.n_pages as usize * 4096);
    let ptr = range.start.0 as *mut () as *mut T;
    unsafe { ptr.write(data) };
}

///# Safety
///the virtual address offset must be correct
#[inline]
pub unsafe fn memset_at_addr<P: Into<VirtRange>>(addr: P, value: u8, size: usize) {
    let range: VirtRange = addr.into();
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
///Arguments must follow args in `std::alloc::Layout::from_size_align(size, align)`
pub unsafe fn align_manual(addr: VirtAddr, size: u64, align: u64) -> VirtAddr {
    if addr.0.is_multiple_of(align) {
        addr
    } else {
        let heap_data =
            unsafe { std::alloc::alloc::alloc(Layout::from_size_align(size as usize, align as usize).expect("Invalid args")) };
        unsafe {
            core::ptr::copy_nonoverlapping(addr.0 as *const u8, heap_data, size as usize);
        }
        VirtAddr(heap_data as u64)
    }
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

impl Drop for OwnedVirtAddr {
    fn drop(&mut self) {
        let range = VirtRange {
            start: self.0,
            n_pages: 1,
        };
        drop(OwnedVirtRange(range));
    }
}

impl Drop for OwnedVirtRange {
    fn drop(&mut self) {
        let phys_addr =
            translate_virt_phys_addr(self.0.start, None).expect("Failed to translate virtual address to physical address");
        if !is_in_hhdm(self.0.start) {
            unsafe { kernel_manual_unmap(self.0.start, self.0.n_pages, None) };
        }
        drop(OwnedPhysRange(PhysRange {
            start: phys_addr,
            n_pages: self.0.n_pages,
        }));
    }
}

impl OwnedVirtRange {
    pub fn into_owned_virt_addr(self) -> OwnedVirtAddr {
        assert!(
            self.0.n_pages == 1,
            "Cannot convert a range with more than one page into an address"
        );
        let addr = self.0.start;
        core::mem::forget(self);
        OwnedVirtAddr(addr)
    }
}
