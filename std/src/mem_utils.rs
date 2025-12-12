use core::alloc::Layout;

static mut PHYSICAL_OFFSET: PhysOffset = PhysOffset(0);
static mut MEM_INITIALIZED: bool = false;
static mut HEAP_INITIALIZED: bool = false;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PhysOffset(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PhysAddr(pub u64);

impl core::ops::Add for PhysAddr {
    type Output = PhysAddr;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::convert::From<VirtAddr> for u64 {
    fn from(value: VirtAddr) -> Self {
        value.0
    }
}

impl core::ops::Add<u64> for PhysAddr {
    type Output = PhysAddr;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::AddAssign for PhysAddr {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl core::ops::AddAssign<u64> for PhysAddr {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs
    }
}

impl core::ops::Add<PhysOffset> for PhysAddr {
    type Output = VirtAddr;
    #[inline]
    fn add(self, rhs: PhysOffset) -> Self::Output {
        VirtAddr(self.0 + rhs.0)
    }
}

impl core::ops::Sub<u64> for PhysAddr {
    type Output = PhysAddr;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        PhysAddr(self.0 - rhs)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(C)]
pub struct VirtAddr(pub u64);

impl core::ops::Add for VirtAddr {
    type Output = VirtAddr;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::AddAssign for VirtAddr {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl core::ops::Add<u64> for VirtAddr {
    type Output = VirtAddr;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u64> for VirtAddr {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::Sub<u64> for VirtAddr {
    type Output = VirtAddr;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0 - rhs)
    }
}

///# Safety
///the address must be valid and there are no other references to the data
#[inline]
pub unsafe fn get_at_physical_addr<T>(addr: PhysAddr) -> &'static mut T {
    unsafe {
        debug_assert!(MEM_INITIALIZED);
        let data: *mut T = (addr + PHYSICAL_OFFSET).0 as *mut T;
        &mut *data
    }
}

///# Safety
///must be a valid addr (with no other data there)
#[inline]
pub unsafe fn set_at_physical_addr<T>(addr: PhysAddr, data: T) {
    unsafe {
        #[cfg(debug_assertions)]
        assert!(MEM_INITIALIZED);
        set_at_virtual_addr(addr + PHYSICAL_OFFSET, data);
    }
}

#[inline]
pub fn get_physical_offset() -> PhysOffset {
    unsafe {
        #[cfg(debug_assertions)]
        assert!(MEM_INITIALIZED);
        PHYSICAL_OFFSET
    }
}

///# Safety
///Can only be called on in-memory read only STATIC data structures
pub unsafe fn ensure_aligned<T>(addr: VirtAddr) -> VirtAddr {
    let align = core::mem::align_of::<T>() as u64;
    let size = core::mem::size_of::<T>() as u64;
    unsafe { ensure_aligned_manual(addr, size, align) }
}
///# Safety
///Can only be called on in-memory read only STATIC data structures
pub unsafe fn ensure_aligned_manual(addr: VirtAddr, size: u64, align: u64) -> VirtAddr {
    if addr.0 % align == 0 {
        addr
    } else {
        let heap_data = unsafe { alloc::alloc::alloc(
            Layout::from_size_align(size as usize, align as usize).unwrap()
        )};
        unsafe {
            core::ptr::copy_nonoverlapping(
                addr.0 as *const u8,
                heap_data,
                size as usize
            );
        }
        VirtAddr(heap_data as u64)
    }
}

///# Safety
///the address must be valid and there are no other references to the data
///This can be used as is if all the data has been written before using the function,
///becasue rust cannot rearrange memory reads when it comes to pointers (which are used)
///If data nges after this function is called, a read_volatile needs to be used
#[inline]
pub unsafe fn get_at_virtual_addr<T>(addr: VirtAddr) -> &'static mut T {
    let data: *mut T = addr.0 as *mut T;
    unsafe { &mut *data }
}

///# Safety
///must be a valid addr (with no other data there)
#[inline]
pub unsafe fn set_at_virtual_addr<T>(addr: VirtAddr, data: T) {
    let data_to_replace: *mut T = addr.0 as *mut T;
    unsafe { data_to_replace.write_volatile(data) };
}

///# Safety
///the physical address offset must be correct
#[inline]
pub unsafe fn set_physical_offset(addr: PhysOffset) {
    unsafe {
        MEM_INITIALIZED = true;
        PHYSICAL_OFFSET = addr;
    }
}

pub fn set_heap_initialized() {
    unsafe { HEAP_INITIALIZED = true };
}

pub fn get_heap_initialized() -> bool {
    unsafe { HEAP_INITIALIZED }
}

///# Safety
///the virtual address offset must be correct
#[inline]
pub unsafe fn memset_virtual_addr(addr: VirtAddr, value: u8, size: usize) {
    let mut data = addr.0 as *mut u8;
    for _ in 0..size {
        unsafe {
            data.write_volatile(value);
            data = data.add(1);
        }
    }
}

///# Safety
///the physical address offset must be correct
#[inline]
pub unsafe fn memset_physical_addr(addr: PhysAddr, value: u8, size: usize) {
    unsafe {
        let virt_addr = addr + PHYSICAL_OFFSET;
        memset_virtual_addr(virt_addr, value, size);
    }
}

///# Safety
///the physical address offset must be correct
#[inline]
pub unsafe fn memcopy_physical_buffer(dest: PhysAddr, src: &[u8]) {
    unsafe {
        let dest_ptr = (dest + PHYSICAL_OFFSET).0 as *mut u8;
        core::ptr::copy_nonoverlapping(src.as_ptr(), dest_ptr, src.len());
    }
}

pub fn translate_virt_phys_addr(addr: VirtAddr, root_page_addr: PhysAddr) -> Option<PhysAddr> {
    let mut page_addr = root_page_addr;
    #[allow(clippy::unusual_byte_groupings)] //they are grouped by section masks
    let mut final_mask: u64 = 0b111111111_111111111_111111111_111111111_111111111111;
    let mask = 0b111_111_111_000;
    for level in (1..5).rev() {
        let offset = PhysAddr((addr.0 >> (level * 9)) & mask);
        final_mask >>= 9;
        let page_entry = unsafe { *get_at_physical_addr::<u64>(page_addr + offset) };
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
pub fn translate_phys_virt_addr(addr: PhysAddr) -> VirtAddr {
    unsafe {
        debug_assert!(MEM_INITIALIZED);
        addr + PHYSICAL_OFFSET
    }
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
