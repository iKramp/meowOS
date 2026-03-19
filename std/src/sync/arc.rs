use core::{fmt::Debug, marker::Unsize, ops::CoerceUnsized, ptr::NonNull};

use alloc::boxed::Box;

#[repr(C)]
pub struct ArcInner<T: ?Sized> {
    ref_count: core::sync::atomic::AtomicUsize,
    data: T,
}

pub struct Arc<T: ?Sized> {
    inner: NonNull<ArcInner<T>>,
}

unsafe impl<T: ?Sized + Send + Sync> Send for Arc<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for Arc<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        let address = Box::into_raw(Box::new(ArcInner {
            ref_count: core::sync::atomic::AtomicUsize::new(1),
            data,
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

    pub fn ref_count(&self) -> usize {
        unsafe { self.inner.as_ref().ref_count.load(core::sync::atomic::Ordering::Relaxed) }
    }
}

impl<T: ?Sized> Clone for Arc<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.inner
                .as_ref()
                .ref_count
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

            Self { inner: self.inner }
        }
    }
}

impl<T> Debug for Arc<T>
where
    T: Debug + ?Sized,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            f.debug_struct("Arc")
                .field("data", &&self.inner.as_ref().data)
                .field(
                    "ref_count",
                    &self.inner.as_ref().ref_count.load(core::sync::atomic::Ordering::Relaxed),
                )
                .finish()
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
        unsafe {
            if self
                .inner
                .as_ref()
                .ref_count
                .fetch_sub(1, core::sync::atomic::Ordering::Relaxed)
                == 1
            {
                let address = self.inner.as_ptr();
                let _ = Box::from_raw(address);
            }
        }
    }
}

impl<T: ?Sized, U: ?Sized> CoerceUnsized<Arc<U>> for Arc<T> where T: Unsize<U> {}
