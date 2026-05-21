use core::{
    fmt::Debug,
    marker::Unsize,
    mem::ManuallyDrop,
    ops::{CoerceUnsized, Deref},
    ptr::{self, NonNull},
    sync::atomic::Ordering,
};

use alloc::boxed::Box;

#[repr(C)]
pub struct ArcInner<T: ?Sized> {
    strong_count: core::sync::atomic::AtomicUsize,
    all_ref_count: core::sync::atomic::AtomicUsize,
    data: ManuallyDrop<T>,
}

pub struct Arc<T: ?Sized> {
    inner: NonNull<ArcInner<T>>,
}

pub struct Weak<T: ?Sized> {
    inner: NonNull<ArcInner<T>>,
}

unsafe impl<T: ?Sized + Send + Sync> Send for Arc<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for Arc<T> {}

unsafe impl<T: ?Sized + Send + Sync> Send for Weak<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for Weak<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        let address = Box::into_raw(Box::new(ArcInner {
            strong_count: core::sync::atomic::AtomicUsize::new(1),
            all_ref_count: core::sync::atomic::AtomicUsize::new(1),
            data: ManuallyDrop::new(data),
        })) as usize;
        Self {
            inner: NonNull::new(address as *mut ArcInner<T>).unwrap(),
        }
    }
}

impl<T: ?Sized> Arc<T> {
    pub fn get(&self) -> &T {
        unsafe { &(self.inner.as_ref().data) }
    }

    pub fn strong_ref_count(&self) -> usize {
        unsafe { self.inner.as_ref().strong_count.load(core::sync::atomic::Ordering::Relaxed) }
    }

    pub fn all_ref_count(&self) -> usize {
        unsafe { self.inner.as_ref().all_ref_count.load(core::sync::atomic::Ordering::Relaxed) }
    }

    pub fn downgrade(&self) -> Weak<T> {
        unsafe {
            self.inner
                .as_ref()
                .all_ref_count
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }
        Weak { inner: self.inner }
    }
}

impl<T: ?Sized> Weak<T> {
    pub fn upgrade(&self) -> Option<Arc<T>> {
        let mut current = unsafe { self.inner.as_ref().strong_count.load(Ordering::Relaxed) };
        loop {
            if current == 0 {
                return None;
            }
            let res = unsafe {
                self.inner
                    .as_ref()
                    .strong_count
                    .compare_exchange(current, current + 1, Ordering::Relaxed, Ordering::Relaxed)
            };
            let Err(n) = res else {
                break;
            };
            current = n;
        }

        unsafe { self.inner.as_ref().all_ref_count.fetch_add(1, Ordering::Relaxed) };

        Some(Arc { inner: self.inner })
    }
}

impl<T: ?Sized> Clone for Arc<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.inner
                .as_ref()
                .all_ref_count
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            self.inner
                .as_ref()
                .strong_count
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

            Self { inner: self.inner }
        }
    }
}

impl<T: ?Sized> Clone for Weak<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.inner
                .as_ref()
                .all_ref_count
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

            Self { inner: self.inner }
        }
    }
}

impl<T> Debug for Arc<T>
where
    T: Debug + ?Sized,
{
    //only informational, data may be out of sync
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            f.debug_struct("Arc")
                .field("data", &&self.inner.as_ref().data)
                .field("all_ref_count", &self.all_ref_count())
                .field("strong_ref_count", &self.strong_ref_count())
                .finish()
        }
    }
}

impl<T> Debug for Weak<T>
where
    T: Debug + ?Sized,
{
    //only informational, data may be out of sync
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            let mut dbg_struct = f.debug_struct("Weak");
            dbg_struct
                .field("all_ref_count", &self.inner.as_ref().all_ref_count.load(Ordering::Relaxed))
                .field("strong_ref_count", &self.inner.as_ref().strong_count.load(Ordering::Relaxed));
            if let Some(data) = self.upgrade() {
                dbg_struct.field("data", &data.get()).finish()
            } else {
                dbg_struct.finish()
            }
        }
    }
}

impl<T: ?Sized> core::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.inner.as_ref().data }
    }
}

impl<T: ?Sized> Drop for Arc<T> {
    fn drop(&mut self) {
        let inner = unsafe { self.inner.as_ref() };
        let prev = inner.strong_count.fetch_sub(1, Ordering::Release);
        if prev == 1 {
            let inner_data = inner.data.deref();
            unsafe { ptr::drop_in_place(inner_data as *const T as *mut T) };
        }
        let prev = inner.all_ref_count.fetch_sub(1, Ordering::Release);
        unsafe {
            if prev == 1 {
                let address = self.inner.as_ptr();
                let _ = Box::from_raw(address);
            }
        }
    }
}

impl<T: ?Sized> Drop for Weak<T> {
    fn drop(&mut self) {
        let prev = unsafe { self.inner.as_ref().all_ref_count.fetch_sub(1, Ordering::Release) };
        unsafe {
            if prev == 1 {
                let address = self.inner.as_ptr();
                let _ = Box::from_raw(address);
            }
        }
    }
}

impl<T: ?Sized, U: ?Sized> CoerceUnsized<Arc<U>> for Arc<T> where T: Unsize<U> {}
