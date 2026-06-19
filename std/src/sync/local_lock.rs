use core::{
    cell::{Cell, UnsafeCell},
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use super::lock_info::LockLocationInfo;

#[repr(C)]
pub struct LocalLock<T: ?Sized> {
    state: Cell<u8>,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for LocalLock<T> {}
unsafe impl<T: ?Sized + Send> Sync for LocalLock<T> {}

pub struct LocalLockReadGuard<'a, T: ?Sized + 'a> {
    location: LockLocationInfo,
    lock: &'a LocalLock<T>,
}
unsafe impl<T: ?Sized> Send for LocalLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for LocalLockReadGuard<'_, T> {}

pub struct LocalLockWriteGuard<'a, T: ?Sized + 'a> {
    location: LockLocationInfo,
    lock: &'a LocalLock<T>,
}
unsafe impl<T: ?Sized> Send for LocalLockWriteGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for LocalLockWriteGuard<'_, T> {}

impl<T> LocalLock<T> {
    pub const fn new(t: T) -> Self {
        Self {
            state: Cell::new(0),
            data: UnsafeCell::new(t),
        }
    }
}

#[macro_export]
macro_rules! local_lock_read_w_info {
    ($l:expr) => {
        $l.lock_read($crate::sync::lock_info::LockLocationInfo(file!(), line!(), column!()))
    };
}

#[macro_export]
macro_rules! local_lock_write_w_info {
    ($l:expr) => {
        $l.lock_write($crate::sync::lock_info::LockLocationInfo(file!(), line!(), column!()))
    };
}

impl<T: ?Sized> LocalLock<T> {
    pub fn lock_read(&self, location: LockLocationInfo) -> LocalLockReadGuard<'_, T> {
        let prev_rflags: u64;
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) prev_rflags,
            );
        }
        let prev_int_state = (prev_rflags & (1 << 9)) != 0;

        let current_state = self.state.get();
        if current_state == u8::MAX {
            panic!("write lock held, fix your damn kernel");
        }
        if current_state == u8::MAX - 1 {
            panic!("lock recursion detected, fix your damn kernel");
        }
        self.state.set(current_state + 1);

        // Safety:
        // interrupts are disabled, and it is from CPU local
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        info.inc_spinlocks(prev_int_state, location.clone());
        LocalLockReadGuard { location, lock: self }
    }

    pub fn lock_write(&self, location: LockLocationInfo) -> LocalLockWriteGuard<'_, T> {
        let prev_rflags: u64;
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) prev_rflags,
            );
        }
        let prev_int_state = (prev_rflags & (1 << 9)) != 0;

        let current_state = self.state.get();

        if current_state != 0 {
            panic!("lock already held while trying to get write lock");
        }

        self.state.set(u8::MAX);

        // Safety:
        // interrupts are disabled, and it is from CPU local
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        info.inc_spinlocks(prev_int_state, location.clone());
        LocalLockWriteGuard { location, lock: self }
    }

    /// only use this in a panic handler
    pub fn force_get_lock(&self) -> LocalLockWriteGuard<'_, T> {
        let prev_rflags: u64;
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) prev_rflags,
            );
        }

        let location = LockLocationInfo("", 0, 0);

        // Safety:
        // interrupts are disabled, and it is from CPU local
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        info.inc_spinlocks((prev_rflags & (1 << 9)) != 0, location.clone());
        LocalLockWriteGuard { location, lock: self }
    }

    ///function to get a poitner to read only data in T
    /// # Safety
    ///
    /// This is unsafe as the caller must ensure that data is never modified and there are no issues
    /// with out of sync reads
    pub unsafe fn get_read_ptr(&self) -> &T {
        unsafe { &*self.data.get() }
    }
}

impl<T: ?Sized> Drop for LocalLockReadGuard<'_, T> {
    fn drop(&mut self) {
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        let should_enable_ints = info.dec_spinlocks(&self.location);

        let previous = self.lock.state.get();

        self.lock.state.set(previous - 1);
        if should_enable_ints {
            unsafe { core::arch::asm!("sti") };
        }
    }
}

impl<T: ?Sized> Drop for LocalLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        let should_enable_ints = info.dec_spinlocks(&self.location);

        self.lock.state.set(0);
        if should_enable_ints {
            unsafe { core::arch::asm!("sti") };
        }
    }
}

impl<T: ?Sized> DerefMut for LocalLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Deref for LocalLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Deref for LocalLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: Default> Default for LocalLock<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: ?Sized + Debug> Debug for LocalLock<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe { f.debug_struct("NoIntSpinlock").field("data", &&*self.data.get()).finish() }
    }
}
