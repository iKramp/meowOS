use core::{
    cell::UnsafeCell,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU8, Ordering::*},
};

use super::lock_info::LockLocationInfo;

pub struct NoIntSpinlock<T: ?Sized> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for NoIntSpinlock<T> {}
unsafe impl<T: ?Sized + Send> Sync for NoIntSpinlock<T> {}

pub struct NoIntSpinlockGuard<'a, T: ?Sized + 'a> {
    location: LockLocationInfo,
    lock: &'a NoIntSpinlock<T>,
}
unsafe impl<T: ?Sized> Send for NoIntSpinlockGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for NoIntSpinlockGuard<'_, T> {}

impl<T> NoIntSpinlock<T> {
    pub const fn new(t: T) -> Self {
        Self {
            state: AtomicU8::new(0),
            data: UnsafeCell::new(t),
        }
    }
}

#[macro_export]
macro_rules! lock_w_info {
    ($l:expr) => {
        $l.lock($crate::sync::lock_info::LockLocationInfo(file!(), line!(), column!()))
    };
}

impl<T: ?Sized> NoIntSpinlock<T> {
    pub fn lock(&self, location: LockLocationInfo) -> NoIntSpinlockGuard<'_, T> {
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
        while self.state.compare_exchange(0, 1, Acquire, Relaxed).is_err() {
            core::hint::spin_loop();
        }
        // Safety:
        // interrupts are disabled, and it is from CPU local
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        info.inc_spinlocks(prev_int_state, location.clone());
        NoIntSpinlockGuard { location, lock: self }
    }

    /// only use this in a panic handler
    pub fn force_get_lock(&self) -> NoIntSpinlockGuard<'_, T> {
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
        NoIntSpinlockGuard { location, lock: self }
    }
}

impl<T: ?Sized> Drop for NoIntSpinlockGuard<'_, T> {
    fn drop(&mut self) {
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        let should_enable_ints = info.dec_spinlocks(&self.location);
        self.lock.state.store(0, Release);
        if should_enable_ints {
            unsafe { core::arch::asm!("sti") };
        }
    }
}

impl<T: ?Sized> DerefMut for NoIntSpinlockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Deref for NoIntSpinlockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: Default> Default for NoIntSpinlock<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: ?Sized + Debug> Debug for NoIntSpinlock<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe { f.debug_struct("NoIntSpinlock").field("data", &&*self.data.get()).finish() }
    }
}
