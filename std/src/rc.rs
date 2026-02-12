use core::{cell::Cell, fmt::Debug, marker::Unsize, ops::CoerceUnsized, ptr::NonNull};

use alloc::boxed::Box;

#[repr(C)]
struct RcInner<T: ?Sized> {
    count: Cell<usize>,
    data: T,
}

pub struct Rc<T>
where
    T: ?Sized,
{
    inner: NonNull<RcInner<T>>,
}

impl<T> Rc<T> {
    pub fn new(data: T) -> Self {
        let address = Box::into_raw(Box::new(RcInner {
            data,
            count: Cell::new(1),
        })) as usize;
        Self {
            inner: NonNull::new(address as *mut RcInner<T>).expect("OOM"),
        }
    }
}

impl<T: ?Sized> Rc<T> {
    pub fn get(&self) -> &T {
        unsafe { &self.inner.as_ref().data }
    }

    pub fn ref_count(&self) -> usize {
        unsafe { self.inner.as_ref().count.get() }
    }
}

impl<T: ?Sized> Drop for Rc<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let cnt = self.inner.as_ref().count.get();
            self.inner.as_ref().count.set(cnt - 1);
            if cnt == 1 {
                let address = self.inner.as_ptr();
                let _ = Box::from_raw(address);
            }
        }
    }
}

impl<T: Debug + ?Sized> Debug for Rc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            f.debug_struct("Rc")
                .field("data", &&self.inner.as_ref().data)
                .field("ref_count", &self.inner.as_ref().count)
                .finish()
        }
    }
}

impl<T: ?Sized> Clone for Rc<T> {
    #[inline]
    fn clone(&self) -> Self {
        unsafe {
            self.inner.as_ref().count.set(self.inner.as_ref().count.get() + 1);
        }

        Self { inner: self.inner }
    }
}

impl<T: ?Sized, U: ?Sized> CoerceUnsized<Rc<U>> for Rc<T> where T: Unsize<U> {}
