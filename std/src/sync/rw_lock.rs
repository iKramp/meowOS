use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU16, Ordering::*},
};

use super::lock_info::LockLocationInfo;

#[derive(Debug)]
pub struct RWSpinlock<T: ?Sized> {
    //highest bit is write lock, lower 15 bits are read lock count
    lock: AtomicU16,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RWSpinlock<T> {}
unsafe impl<T: ?Sized + Send> Sync for RWSpinlock<T> {}

pub struct RWLockModeRead;
pub struct RWLockModeWrite;

pub struct RWSpinlockGuard<'a, T: ?Sized + 'a, M> {
    lock: &'a RWSpinlock<T>,
    marker: core::marker::PhantomData<M>,
    location: LockLocationInfo,
}
unsafe impl<T: ?Sized, M> Send for RWSpinlockGuard<'_, T, M> {}
unsafe impl<T: ?Sized + Sync, M> Sync for RWSpinlockGuard<'_, T, M> {}

impl<T> RWSpinlock<T> {
    pub const fn new(t: T) -> Self {
        Self {
            lock: AtomicU16::new(0),
            data: UnsafeCell::new(t),
        }
    }
}

#[macro_export]
macro_rules! r_lock_w_info {
    ($l:expr) => {
        $l.lock_read($crate::sync::lock_info::LockLocationInfo(file!(), line!(), column!()))
    };
}
#[macro_export]
macro_rules! w_lock_w_info {
    ($l:expr) => {
        $l.lock_write($crate::sync::lock_info::LockLocationInfo(file!(), line!(), column!()))
    };
}

impl<T: ?Sized> RWSpinlock<T> {
    pub fn lock_read(&self, location: LockLocationInfo) -> RWSpinlockGuard<'_, T, RWLockModeRead> {
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
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
        loop {
            let mut state = self.lock.load(Relaxed);
            while (state & 0x8000) != 0 {
                core::hint::spin_loop();
                state = self.lock.load(Relaxed);
            }
            if self.lock.compare_exchange(state, state + 1, Acquire, Relaxed).is_ok() {
                // Safety:
                // interrupts are disabled, and it is from CPU local
                info.inc_spinlocks(prev_int_state, location.clone());
                return RWSpinlockGuard {
                    lock: self,
                    marker: core::marker::PhantomData,
                    location,
                };
            }
        }
    }
    pub fn lock_write(&self, location: LockLocationInfo) -> RWSpinlockGuard<'_, T, RWLockModeWrite> {
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
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
        loop {
            let mut state = self.lock.load(Relaxed);
            while state != 0 {
                core::hint::spin_loop();
                state = self.lock.load(Relaxed);
            }
            if self.lock.compare_exchange(0, 0x8000, Acquire, Relaxed).is_ok() {
                // Safety:
                // interrupts are disabled, and it is from CPU local
                info.inc_spinlocks(prev_int_state, location.clone());
                return RWSpinlockGuard {
                    lock: self,
                    marker: core::marker::PhantomData,
                    location,
                };
            }
        }
    }
}

impl<T: ?Sized, M> RWSpinlockGuard<'_, T, M> {
    pub fn is_only_lock(&self) -> bool {
        let status = self.lock.lock.load(Relaxed);
        status == 0x8000 || status == 1
    }
}

impl<'a, T: ?Sized> RWSpinlockGuard<'a, T, RWLockModeRead> {
    pub fn upgrade_to_write(self) -> RWSpinlockGuard<'a, T, RWLockModeWrite> {
        let state = self.lock.lock.load(Relaxed);
        debug_assert!(state & 0x8000 == 0); //not already write locked
        debug_assert!(state >= 1); //at least one read lock held

        self.lock.lock.fetch_or(0x8000, Acquire); //set write lock bit

        self.lock.lock.fetch_sub(1, Relaxed); //release read lock

        //wait for other readers to release
        while self.lock.lock.load(Relaxed) != 0x8000 {
            core::hint::spin_loop();
        }

        let location = self.location.clone();

        let new_lock = RWSpinlockGuard {
            lock: self.lock,
            marker: core::marker::PhantomData,
            location,
        };
        core::mem::forget(self); //don't run drop
        new_lock
    }
}

impl<'a, T: ?Sized> RWSpinlockGuard<'a, T, RWLockModeWrite> {
    pub fn downgrade_to_read(self) -> RWSpinlockGuard<'a, T, RWLockModeRead> {
        let state = self.lock.lock.load(Relaxed);
        debug_assert!(state & 0x8000 == 0x8000); //is write locked

        self.lock.lock.store(1, Release); //release write lock, acquire read lock

        let location = self.location.clone();

        let new_lock = RWSpinlockGuard {
            lock: self.lock,
            marker: core::marker::PhantomData,
            location,
        };
        core::mem::forget(self); //don't run drop
        new_lock
    }
}

impl<T: ?Sized, M> Drop for RWSpinlockGuard<'_, T, M> {
    fn drop(&mut self) {
        let info = unsafe { super::lock_info::GET_LOCK_INFO() };
        let state = self.lock.lock.load(Relaxed);
        let should_enable_ints = info.dec_spinlocks(&self.location);
        if state & 0x8000 != 0 {
            //write lock
            self.lock.lock.store(0, Release);
        } else {
            self.lock.lock.fetch_sub(1, Relaxed);
        }
        if should_enable_ints {
            unsafe { core::arch::asm!("sti") };
        }
    }
}

impl<T: ?Sized> DerefMut for RWSpinlockGuard<'_, T, RWLockModeWrite> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized, M> Deref for RWSpinlockGuard<'_, T, M> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}
