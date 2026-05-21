use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::atomic::{AtomicU8, Ordering::*},
    task::{Context, Poll},
};

use alloc::boxed::Box;

use crate::lock_w_info;

use super::no_int_spinlock::NoIntSpinlock;

#[derive(Debug)]
pub struct AsyncSpinlock<T: ?Sized> {
    state: AtomicU8,
    wakers: NoIntSpinlock<Option<Box<WakerNode>>>,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for AsyncSpinlock<T> {}
unsafe impl<T: ?Sized + Send> Sync for AsyncSpinlock<T> {}

pub struct AsyncSpinlockGuard<'a, T: ?Sized + 'a> {
    lock: &'a AsyncSpinlock<T>,
}
unsafe impl<T: ?Sized> Send for AsyncSpinlockGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for AsyncSpinlockGuard<'_, T> {}

struct AsyncSpinLockFuture<'a, T: ?Sized + 'a> {
    lock: &'a AsyncSpinlock<T>,
}

#[derive(Debug)]
struct WakerNode {
    waker: core::task::Waker,
    next: Option<Box<WakerNode>>,
}

impl<T> AsyncSpinlock<T> {
    pub const fn new(t: T) -> Self {
        Self {
            state: AtomicU8::new(0),
            data: UnsafeCell::new(t),
            wakers: NoIntSpinlock::new(None),
        }
    }

    pub async fn lock(&self) -> AsyncSpinlockGuard<'_, T> {
        AsyncSpinLockFuture { lock: self }.await
    }

    ///function to get a poitner to read only data in T
    /// # Safety
    ///
    /// This is unsafe as the caller must ensure that data is never modified
    pub unsafe fn get_read_ptr(&self) -> &T {
        unsafe { &*self.data.get() }
    }
}

impl<'a, T: 'a> Future for AsyncSpinLockFuture<'a, T> {
    type Output = AsyncSpinlockGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.lock.state.compare_exchange(0, 1, Acquire, Relaxed).is_ok() {
            Poll::Ready(AsyncSpinlockGuard { lock: self.lock })
        } else {
            let info = unsafe { super::lock_info::GET_LOCK_INFO() };
            if !info.is_blocking_task() {
                //waking executor
                let mut wakers = lock_w_info!(self.lock.wakers);
                let new_node = Box::new(WakerNode {
                    waker: cx.waker().clone(),
                    next: wakers.take(),
                });
                *wakers = Some(new_node);
            }
            Poll::Pending
        }
    }
}

impl<T: ?Sized> Drop for AsyncSpinlockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.store(0, Release);
        //wake 1 waiting task
        let mut wakers = lock_w_info!(self.lock.wakers);
        if let Some(node) = wakers.take() {
            if let Some(next_node) = node.next {
                *wakers = Some(next_node);
            }
            node.waker.wake();
        }
    }
}

impl<T: ?Sized> DerefMut for AsyncSpinlockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Deref for AsyncSpinlockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}
