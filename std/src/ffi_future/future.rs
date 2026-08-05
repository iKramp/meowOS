use alloc::boxed::Box;

use crate::ffi_future::wake::Waker;
use core::future::Future as StdFuture;
use core::pin::Pin;
use core::ptr::NonNull;

#[repr(C)]
pub enum Poll<T> {
    Ready(T),
    Pending,
}

impl<T> Poll<T> {
    pub fn is_ready(&self) -> bool {
        matches!(self, Poll::Ready(_))
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Poll::Pending)
    }
}

#[repr(C)]
pub struct Future<'a, Output> {
    pub data: NonNull<()>, //thin pointer to data only, not to dyn object or anything else
    pub poll_fn: unsafe extern "C" fn(NonNull<()>, &Waker) -> Poll<Output>,
    pub drop_fn: unsafe extern "C" fn(NonNull<()>),
    lifetime: core::marker::PhantomData<&'a ()>,
}

pub fn into_ffi_future<'a, F>(fut: F) -> Future<'a, F::Output>
where
    F: StdFuture + 'a,
{
    unsafe extern "C" fn poll_impl<'b, F>(data: NonNull<()>, waker: &Waker) -> Poll<F::Output>
    where
        F: StdFuture + 'b,
    {
        let std_waker = waker.clone().into_std_waker();
        let mut context = core::task::Context::from_waker(&std_waker);

        let res = unsafe { Pin::new_unchecked(&mut *(data.as_ptr() as *mut F)).poll(&mut context) };
        match res {
            core::task::Poll::Ready(output) => Poll::Ready(output),
            core::task::Poll::Pending => Poll::Pending,
        }
    }

    unsafe extern "C" fn drop_impl<'b, F>(data: NonNull<()>)
    where
        F: StdFuture + 'b,
    {
        // reconstruct the boxed future and drop it
        // # Safety: safe because data was in a box before 'leaked' so it was on heap
        let _ = unsafe { Box::from_raw(data.as_ptr() as *mut F) };
    }

    let boxed = Box::new(fut);

    Future {
        data: unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut ()) },
        poll_fn: poll_impl::<F>,
        drop_fn: drop_impl::<F>,
        lifetime: core::marker::PhantomData,
    }
}

impl<Output> Future<'_, Output> {
    pub fn poll(&mut self, waker: &Waker) -> Poll<Output> {
        // # Safety: Data is owned and validated at construction time
        unsafe { (self.poll_fn)(self.data, waker) }
    }
}

impl<Output> Drop for Future<'_, Output> {
    fn drop(&mut self) {
        // # Safety: Data is owned and validated at construction time
        unsafe { (self.drop_fn)(self.data) }
    }
}
