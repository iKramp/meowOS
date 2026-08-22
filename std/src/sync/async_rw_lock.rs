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

struct AsyncRWLockWriteWaitAfterSetFuture<'a, T: ?Sized + 'a> {
    upgrading: bool,
    lock: &'a AsyncRWlock<T>,
}

struct AsyncRWLockWriteWaitBeforeSetFuture<'a, T: ?Sized + 'a> {
    lock: &'a AsyncRWlock<T>,
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
        AsyncRWLockWriteWaitBeforeSetFuture { lock: self }.await;
        AsyncRWLockWriteWaitAfterSetFuture {
            lock: self,
            upgrading: false,
        }
        .await
    }
}

impl<'a, T: 'a> AsyncRWlockGuard<'a, T, AsyncRWLockModeRead> {
    pub async fn upgrade_to_write(self) -> AsyncRWlockGuard<'a, T, AsyncRWLockModeWrite> {
        let lock = &self.lock;
        AsyncRWLockWriteWaitBeforeSetFuture { lock }.await;
        AsyncRWLockWriteWaitAfterSetFuture { lock, upgrading: true }.await
    }
}

impl<'a, T: 'a> AsyncRWlockGuard<'a, T, AsyncRWLockModeWrite> {
    pub fn downgrade_to_read(self) -> AsyncRWlockGuard<'a, T, AsyncRWLockModeRead> {
        //guaranteed to be the only writer and no readers
        let guard = AsyncRWlockGuard {
            lock: self.lock,
            marker: core::marker::PhantomData,
        };
        self.lock.state.store(1, SeqCst);
        wake_all_tasks(self.lock);
        core::mem::forget(self); //don't try to drop
        guard
    }
}

impl<'a, T: 'a> Future for AsyncRWLockFuture<'a, T, AsyncRWLockModeRead> {
    type Output = AsyncRWlockGuard<'a, T, AsyncRWLockModeRead>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = self.lock.state.load(Relaxed);
        if (state & 0x8000) != 0 {
            //write locked
            wait(self.lock, cx);
            return Poll::Pending;
        }
        if self.lock.state.compare_exchange(state, state + 1, Acquire, Relaxed).is_ok() {
            Poll::Ready(AsyncRWlockGuard {
                lock: self.lock,
                marker: core::marker::PhantomData,
            })
        } else {
            wait(self.lock, cx);
            Poll::Pending
        }
    }
}

impl<'a, T: 'a> Future for AsyncRWLockWriteWaitBeforeSetFuture<'a, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let current = self.lock.state.load(Relaxed);
            if current & 0x8000 == 0x8000 {
                //locked
                wait(self.lock, cx);
                return Poll::Pending;
            }

            let new = current | 0x8000;
            let success = self.lock.state.compare_exchange(current, new, Acquire, Relaxed);
            if success.is_ok() {
                return Poll::Ready(());
            }
        }
    }
}

//upgrading means a read guard must be released after this guard is returned and before it is used
impl<'a, T: 'a> Future for AsyncRWLockWriteWaitAfterSetFuture<'a, T> {
    type Output = AsyncRWlockGuard<'a, T, AsyncRWLockModeWrite>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let target = if self.upgrading { 0x8001 } else { 0x8000 };

        if self.lock.state.load(Acquire) != target {
            wait(self.lock, cx);
            Poll::Pending
        } else {
            Poll::Ready(AsyncRWlockGuard {
                lock: self.lock,
                marker: core::marker::PhantomData,
            })
        }
    }
}

fn wait<T>(lock: &AsyncRWlock<T>, cx: &mut Context<'_>) {
    let lock_info = unsafe { super::lock_info::GET_LOCK_INFO() };
    if !lock_info.is_blocking_task() {
        //waking executor
        let mut wakers = lock_w_info!(lock.wakers);
        let new_node = Box::new(WakerNode {
            waker: cx.waker().clone(),
            next: wakers.take(),
        });
        *wakers = Some(new_node);
    }
}

impl<T: ?Sized, M> Drop for AsyncRWlockGuard<'_, T, M> {
    fn drop(&mut self) {
        let current_state = self.lock.state.load(Relaxed);
        if current_state == 0x8000 {
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
        wake_all_tasks(self.lock);
    }
}

fn wake_all_tasks<T: ?Sized>(lock: &AsyncRWlock<T>) {
    let mut wakers = lock_w_info!(lock.wakers);
    while let Some(node) = wakers.take() {
        if let Some(next_node) = node.next {
            *wakers = Some(next_node);
        }
        node.waker.wake();
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
