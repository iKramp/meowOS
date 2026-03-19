use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::atomic::{AtomicU16, Ordering::*},
    task::{Context, Poll},
};

use super::no_int_spinlock::NoIntSpinlock;
use crate::lock_w_info;
use alloc::boxed::Box;

#[derive(Debug)]
pub struct AsyncRWlock<T: ?Sized> {
    state: AtomicU16,
    wakers: NoIntSpinlock<Option<Box<WakerNode>>>,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for AsyncRWlock<T> {}
unsafe impl<T: ?Sized + Send> Sync for AsyncRWlock<T> {}

pub struct AsyncRWLockModeRead;
pub struct AsyncRWLockModeWrite;

pub struct AsyncRWlockGuard<'a, T: ?Sized + 'a, M> {
    lock: &'a AsyncRWlock<T>,
    marker: core::marker::PhantomData<M>,
}
unsafe impl<T: ?Sized, M> Send for AsyncRWlockGuard<'_, T, M> {}
unsafe impl<T: ?Sized + Sync, M> Sync for AsyncRWlockGuard<'_, T, M> {}

struct AsyncRWLockFuture<'a, T: ?Sized + 'a, M> {
    lock: &'a AsyncRWlock<T>,
    marker: core::marker::PhantomData<M>,
}

#[derive(Debug)]
struct WakerNode {
    waker: core::task::Waker,
    next: Option<Box<WakerNode>>,
}

impl<T> AsyncRWlock<T> {
    pub const fn new(t: T) -> Self {
        Self {
            state: AtomicU16::new(0),
            data: UnsafeCell::new(t),
            wakers: NoIntSpinlock::new(None),
        }
    }
}

impl<T> AsyncRWlock<T> {
    #[allow(private_interfaces)] //caller doesn't need to know about Read
    pub async fn lock_read(&self) -> AsyncRWlockGuard<'_, T, AsyncRWLockModeRead> {
        AsyncRWLockFuture {
            lock: self,
            marker: core::marker::PhantomData::<AsyncRWLockModeRead>,
        }
        .await
    }

    #[allow(private_interfaces)] //caller doesn't need to know about Write
    pub async fn lock_write(&self) -> AsyncRWlockGuard<'_, T, AsyncRWLockModeWrite> {
        AsyncRWLockFuture {
            lock: self,
            marker: core::marker::PhantomData::<AsyncRWLockModeWrite>,
        }
        .await
    }
}

impl<'a, T: 'a> Future for AsyncRWLockFuture<'a, T, AsyncRWLockModeRead> {
    type Output = AsyncRWlockGuard<'a, T, AsyncRWLockModeRead>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = self.lock.state.load(Relaxed);
        if (state & 0x8000) != 0 {
            //write locked
            let lock_info = unsafe { super::lock_info::GET_LOCK_INFO() };
            if !lock_info.is_blocking_task() {
                //waking executor
                let mut wakers = lock_w_info!(self.lock.wakers);
                let new_node = Box::new(WakerNode {
                    waker: cx.waker().clone(),
                    next: wakers.take(),
                });
                *wakers = Some(new_node);
            }
            return Poll::Pending;
        }
        if self.lock.state.compare_exchange(state, state + 1, Acquire, Relaxed).is_ok() {
            Poll::Ready(AsyncRWlockGuard {
                lock: self.lock,
                marker: core::marker::PhantomData,
            })
        } else {
            let lock_info = unsafe { super::lock_info::GET_LOCK_INFO() };
            if !lock_info.is_blocking_task() {
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

impl<'a, T: 'a> Future for AsyncRWLockFuture<'a, T, AsyncRWLockModeWrite> {
    type Output = AsyncRWlockGuard<'a, T, AsyncRWLockModeWrite>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.lock.state.compare_exchange(0, 0x8000, Acquire, Relaxed).is_ok() {
            Poll::Ready(AsyncRWlockGuard {
                lock: self.lock,
                marker: core::marker::PhantomData,
            })
        } else {
            let lock_info = unsafe { super::lock_info::GET_LOCK_INFO() };
            if !lock_info.is_blocking_task() {
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

impl<T: ?Sized, M> Drop for AsyncRWlockGuard<'_, T, M> {
    fn drop(&mut self) {
        let current_state = self.lock.state.load(Relaxed);
        if (current_state & 0x8000) == 0x8000 {
            //was write locked
            self.lock.state.store(0, Release);
        } else {
            //was read locked
            self.lock.state.fetch_sub(1, Release);
            if self.lock.state.load(Relaxed) != 0 {
                //other readers still exist
                return;
            }
        }

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

impl<T: ?Sized> DerefMut for AsyncRWlockGuard<'_, T, AsyncRWLockModeWrite> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized, M> Deref for AsyncRWlockGuard<'_, T, M> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}
