use alloc::boxed::Box;

use crate::mem::ManuallyDrop;
use crate::task::RawWaker as StdRawWaker;
use crate::task::RawWakerVTable as StdRawWakerVTable;
use crate::task::Waker as StdWaker;
use crate::{fmt, ptr};

/// A `RawWaker` allows the implementor of a task executor to create a [`Waker`]
/// or a [`LocalWaker`] which provides customized wakeup behavior.
///
/// It consists of a data pointer and a [virtual function pointer table (vtable)][vtable]
/// that customizes the behavior of the `RawWaker`.
///
/// `RawWaker`s are unsafe to use.
/// Implementing the [`Wake`] trait is a safe alternative that requires memory allocation.
///
/// [vtable]: https://en.wikipedia.org/wiki/Virtual_method_table
/// [`Wake`]: ../../alloc/task/trait.Wake.html
#[repr(C)]
#[derive(PartialEq, Debug)]
pub struct RawWaker {
    /// A data pointer, which can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this field gets passed to all functions that are part of
    /// the vtable as the first parameter.
    data: *const (),
    /// Virtual function pointer table that customizes the behavior of this waker.
    vtable: &'static RawWakerVTable,
}

impl RawWaker {
    #[inline]
    #[must_use]
    pub const fn new(data: *const (), vtable: &'static RawWakerVTable) -> RawWaker {
        RawWaker { data, vtable }
    }

    const NOOP: RawWaker = {
        extern "C" fn noop_clone(_: *const ()) -> RawWaker {
            RawWaker::NOOP
        }
        extern "C" fn noop_wake(_: *const ()) {}
        extern "C" fn noop_wake_by_ref(_: *const ()) {}
        extern "C" fn noop_drop(_: *const ()) {}

        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            // Cloning just returns a new no-op raw waker
            noop_clone,
            // `wake` does nothing
            noop_wake,
            // `wake_by_ref` does nothing
            noop_wake_by_ref,
            // Dropping does nothing as we don't allocate anything
            noop_drop,
        );
        RawWaker::new(ptr::null(), &VTABLE)
    };
}

/// A virtual function pointer table (vtable) that specifies the behavior
/// of a [`RawWaker`].
///
/// The pointer passed to all functions inside the vtable is the `data` pointer
/// from the enclosing [`RawWaker`] object.
///
/// The functions inside this struct are only intended to be called on the `data`
/// pointer of a properly constructed [`RawWaker`] object from inside the
/// [`RawWaker`] implementation. Calling one of the contained functions using
/// any other `data` pointer will cause undefined behavior.
///
/// Note that while this type implements `PartialEq`, comparing function pointers, and hence
/// comparing structs like this that contain function pointers, is unreliable: pointers to the same
/// function can compare inequal (because functions are duplicated in multiple codegen units), and
/// pointers to *different* functions can compare equal (since identical functions can be
/// deduplicated within a codegen unit).
///
/// # Thread safety
/// If the [`RawWaker`] will be used to construct a [`Waker`] then
/// these functions must all be thread-safe (even though [`RawWaker`] is
/// <code>\![Send] + \![Sync]</code>). This is because [`Waker`] is <code>[Send] + [Sync]</code>,
/// and it may be moved to arbitrary threads or invoked by `&` reference. For example,
/// this means that if the `clone` and `drop` functions manage a reference count,
/// they must do so atomically.
///
/// However, if the [`RawWaker`] will be used to construct a [`LocalWaker`] instead, then
/// these functions don't need to be thread safe. This means that <code>\![Send] + \![Sync]</code>
///  data can be stored in the data pointer, and reference counting does not need any atomic
/// synchronization. This is because [`LocalWaker`] is not thread safe itself, so it cannot
/// be sent across threads.
#[repr(C)]
#[allow(unpredictable_function_pointer_comparisons)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct RawWakerVTable {
    /// This function will be called when the [`RawWaker`] gets cloned, e.g. when
    /// the [`Waker`] in which the [`RawWaker`] is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a [`RawWaker`] and associated
    /// task. Calling `wake` on the resulting [`RawWaker`] should result in a wakeup
    /// of the same task that would have been awoken by the original [`RawWaker`].
    clone: unsafe extern "C" fn(*const ()) -> RawWaker,

    /// This function will be called when `wake` is called on the [`Waker`].
    /// It must wake up the task associated with this [`RawWaker`].
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawWaker`] and
    /// associated task.
    wake: unsafe extern "C" fn(*const ()),

    /// This function will be called when `wake_by_ref` is called on the [`Waker`].
    /// It must wake up the task associated with this [`RawWaker`].
    ///
    /// This function is similar to `wake`, but must not consume the provided data
    /// pointer.
    wake_by_ref: unsafe extern "C" fn(*const ()),

    /// This function will be called when a [`Waker`] gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawWaker`] and
    /// associated task.
    drop: unsafe extern "C" fn(*const ()),
}

impl RawWakerVTable {
    /// Creates a new `RawWakerVTable` from the provided `clone`, `wake`,
    /// `wake_by_ref`, and `drop` functions.
    ///
    /// If the [`RawWaker`] will be used to construct a [`Waker`] then
    /// these functions must all be thread-safe (even though [`RawWaker`] is
    /// <code>\![Send] + \![Sync]</code>). This is because [`Waker`] is <code>[Send] + [Sync]</code>,
    /// and it may be moved to arbitrary threads or invoked by `&` reference. For example,
    /// this means that if the `clone` and `drop` functions manage a reference count,
    /// they must do so atomically.
    ///
    /// However, if the [`RawWaker`] will be used to construct a [`LocalWaker`] instead, then
    /// these functions don't need to be thread safe. This means that <code>\![Send] + \![Sync]</code>
    /// data can be stored in the data pointer, and reference counting does not need any atomic
    /// synchronization. This is because [`LocalWaker`] is not thread safe itself, so it cannot
    /// be sent across threads.
    /// # `clone`
    ///
    /// This function will be called when the [`RawWaker`] gets cloned, e.g. when
    /// the [`Waker`]/[`LocalWaker`] in which the [`RawWaker`] is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a [`RawWaker`] and associated
    /// task. Calling `wake` on the resulting [`RawWaker`] should result in a wakeup
    /// of the same task that would have been awoken by the original [`RawWaker`].
    ///
    /// # `wake`
    ///
    /// This function will be called when `wake` is called on the [`Waker`].
    /// It must wake up the task associated with this [`RawWaker`].
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawWaker`] and
    /// associated task.
    ///
    /// # `wake_by_ref`
    ///
    /// This function will be called when `wake_by_ref` is called on the [`Waker`].
    /// It must wake up the task associated with this [`RawWaker`].
    ///
    /// This function is similar to `wake`, but must not consume the provided data
    /// pointer.
    ///
    /// # `drop`
    ///
    /// This function will be called when a [`Waker`]/[`LocalWaker`] gets
    /// dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawWaker`] and
    /// associated task.
    pub const fn new(
        clone: unsafe extern "C" fn(*const ()) -> RawWaker,
        wake: unsafe extern "C" fn(*const ()),
        wake_by_ref: unsafe extern "C" fn(*const ()),
        drop: unsafe extern "C" fn(*const ()),
    ) -> Self {
        Self {
            clone,
            wake,
            wake_by_ref,
            drop,
        }
    }
}

/// A `Waker` is a handle for waking up a task by notifying its executor that it
/// is ready to be run.
///
/// This handle encapsulates a [`RawWaker`] instance, which defines the
/// executor-specific wakeup behavior.
///
/// The typical life of a `Waker` is that it is constructed by an executor, wrapped in a
/// [`Context`], then passed to [`Future::poll()`]. Then, if the future chooses to return
/// [`Poll::Pending`], it must also store the waker somehow and call [`Waker::wake()`] when
/// the future should be polled again.
///
/// Implements [`Clone`], [`Send`], and [`Sync`]; therefore, a waker may be invoked
/// from any thread, including ones not in any way managed by the executor. For example,
/// this might be done to wake a future when a blocking function call completes on another
/// thread.
///
/// Note that it is preferable to use `waker.clone_from(&new_waker)` instead
/// of `*waker = new_waker.clone()`, as the former will avoid cloning the waker
/// unnecessarily if the two wakers [wake the same task](Self::will_wake).
///
/// Constructing a `Waker` from a [`RawWaker`] is unsafe.
/// Implementing the [`Wake`] trait is a safe alternative that requires memory allocation.
///
/// [`Future::poll()`]: core::future::Future::poll
/// [`Poll::Pending`]: core::task::Poll::Pending
/// [`Wake`]: ../../alloc/task/trait.Wake.html
#[repr(transparent)]
pub struct Waker {
    waker: RawWaker,
}

impl Unpin for Waker {}
unsafe impl Send for Waker {}
unsafe impl Sync for Waker {}

impl Waker {
    /// Std waker is a wrapper around FFI safe Waker. Data pointer is an owned pointer to Waker
    pub fn into_std_waker(self) -> StdWaker {
        static VTABLE: StdRawWakerVTable =
            StdRawWakerVTable::new(clone_std_waker, wake_std_waker, wake_by_ref_std_waker, drop_std_waker);
        fn clone_std_waker(this: *const ()) -> StdRawWaker {
            let waker = unsafe { &*(this as *const Waker) }; //inner data
            let data = Box::new(waker.clone());
            StdRawWaker::new(Box::into_raw(data) as *const (), &VTABLE)
        }
        fn wake_std_waker(this: *const ()) {
            let waker = unsafe { &*(this as *const Waker) };
            waker.wake_by_ref();
        }
        fn wake_by_ref_std_waker(this: *const ()) {
            let waker = unsafe { &*(this as *const Waker) };
            waker.wake_by_ref();
        }
        fn drop_std_waker(this: *const ()) {
            let _ = unsafe { Box::from_raw(this as *mut Waker) };
        }

        let boxed_waker = Box::new(self);

        unsafe { StdWaker::from_raw(StdRawWaker::new(Box::into_raw(boxed_waker) as *const (), &VTABLE)) }
    }

    pub fn from_std_waker(waker: StdWaker) -> Self {
        //unwrap data
        let inner_data = waker.data();
        let owned_data = unsafe { Box::from_raw(inner_data as *mut Waker) };
        crate::mem::forget(waker); //don't drop the original waker
        *owned_data
    }

    /// Wakes up the task associated with this `Waker`.
    ///
    /// As long as the executor keeps running and the task is not finished, it is
    /// guaranteed that each invocation of [`wake()`](Self::wake) (or
    /// [`wake_by_ref()`](Self::wake_by_ref)) will be followed by at least one
    /// [`poll()`] of the task to which this `Waker` belongs. This makes
    /// it possible to temporarily yield to other tasks while running potentially
    /// unbounded processing loops.
    ///
    /// Note that the above implies that multiple wake-ups may be coalesced into a
    /// single [`poll()`] invocation by the runtime.
    ///
    /// Also note that yielding to competing tasks is not guaranteed: it is the
    /// executor’s choice which task to run and the executor may choose to run the
    /// current task again.
    ///
    /// [`poll()`]: crate::future::Future::poll
    #[inline]
    pub fn wake(self) {
        // The actual wakeup call is delegated through a virtual function call
        // to the implementation which is defined by the executor.

        // Don't call `drop` -- the waker will be consumed by `wake`.
        let this = ManuallyDrop::new(self);

        // SAFETY: This is safe because `Waker::from_raw` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.
        unsafe { (this.waker.vtable.wake)(this.waker.data) };
    }

    /// Wakes up the task associated with this `Waker` without consuming the `Waker`.
    ///
    /// This is similar to [`wake()`](Self::wake), but may be slightly less efficient in
    /// the case where an owned `Waker` is available. This method should be preferred to
    /// calling `waker.clone().wake()`.
    #[inline]
    pub fn wake_by_ref(&self) {
        // The actual wakeup call is delegated through a virtual function call
        // to the implementation which is defined by the executor.

        // SAFETY: see `wake`
        unsafe { (self.waker.vtable.wake_by_ref)(self.waker.data) }
    }

    /// Returns `true` if this `Waker` and another `Waker` would awake the same task.
    ///
    /// This function works on a best-effort basis, and may return false even
    /// when the `Waker`s would awaken the same task. However, if this function
    /// returns `true`, it is guaranteed that the `Waker`s will awaken the same task.
    ///
    /// This function is primarily used for optimization purposes — for example,
    /// this type's [`clone_from`](Self::clone_from) implementation uses it to
    /// avoid cloning the waker when they would wake the same task anyway.
    #[inline]
    #[must_use]
    pub fn will_wake(&self, other: &Waker) -> bool {
        // We optimize this by comparing vtable addresses instead of vtable contents.
        // This is permitted since the function is documented as best-effort.
        let RawWaker {
            data: a_data,
            vtable: a_vtable,
        } = self.waker;
        let RawWaker {
            data: b_data,
            vtable: b_vtable,
        } = other.waker;
        a_data == b_data && ptr::eq(a_vtable, b_vtable)
    }

    /// Creates a new `Waker` from the provided `data` pointer and `vtable`.
    ///
    /// The `data` pointer can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this pointer will get passed to all functions that are part
    /// of the `vtable` as the first parameter.
    ///
    /// It is important to consider that the `data` pointer must point to a
    /// thread safe type such as an `Arc`.
    ///
    /// The `vtable` customizes the behavior of a `Waker`. For each operation
    /// on the `Waker`, the associated function in the `vtable` will be called.
    ///
    /// # Safety
    ///
    /// The behavior of the returned `Waker` is undefined if the contract defined
    /// in [`RawWakerVTable`]'s documentation is not upheld.
    ///
    /// (Authors wishing to avoid unsafe code may implement the [`Wake`] trait instead, at the
    /// cost of a required heap allocation.)
    ///
    /// [`Wake`]: ../../alloc/task/trait.Wake.html
    #[inline]
    #[must_use]
    pub const unsafe fn new(data: *const (), vtable: &'static RawWakerVTable) -> Self {
        Waker {
            waker: RawWaker { data, vtable },
        }
    }

    #[inline]
    #[must_use]
    pub const fn noop() -> &'static Waker {
        const WAKER: &Waker = &Waker { waker: RawWaker::NOOP };
        WAKER
    }

    /// Creates a new `Waker` from [`RawWaker`].
    ///
    /// # Safety
    ///
    /// The behavior of the returned `Waker` is undefined if the contract defined
    /// in [`RawWaker`]'s and [`RawWakerVTable`]'s documentation is not upheld.
    ///
    /// (Authors wishing to avoid unsafe code may implement the [`Wake`] trait instead, at the
    /// cost of a required heap allocation.)
    ///
    /// [`Wake`]: ../../alloc/task/trait.Wake.html
    #[inline]
    #[must_use]
    pub const unsafe fn from_raw(waker: RawWaker) -> Waker {
        Waker { waker }
    }

    /// Gets the `data` pointer used to create this `Waker`.
    #[inline]
    #[must_use]
    pub fn data(&self) -> *const () {
        self.waker.data
    }

    /// Gets the `vtable` pointer used to create this `Waker`.
    #[inline]
    #[must_use]
    pub fn vtable(&self) -> &'static RawWakerVTable {
        self.waker.vtable
    }
}

impl Clone for Waker {
    #[inline]
    fn clone(&self) -> Self {
        Waker {
            // SAFETY: This is safe because `Waker::from_raw` is the only way
            // to initialize `clone` and `data` requiring the user to acknowledge
            // that the contract of [`RawWaker`] is upheld.
            waker: unsafe { (self.waker.vtable.clone)(self.waker.data) },
        }
    }

    /// Assigns a clone of `source` to `self`, unless [`self.will_wake(source)`][Waker::will_wake] anyway.
    ///
    /// This method is preferred over simply assigning `source.clone()` to `self`,
    /// as it avoids cloning the waker if `self` is already the same waker.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::sync::{Arc, Mutex};
    /// use std::task::{Context, Poll, Waker};
    ///
    /// struct Waiter {
    ///     shared: Arc<Mutex<Shared>>,
    /// }
    ///
    /// struct Shared {
    ///     waker: Waker,
    ///     // ...
    /// }
    ///
    /// impl Future for Waiter {
    ///     type Output = ();
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         let mut shared = self.shared.lock().unwrap();
    ///
    ///         // update the waker
    ///         shared.waker.clone_from(cx.waker());
    ///
    ///         // readiness logic ...
    /// #       Poll::Ready(())
    ///     }
    /// }
    ///
    /// ```
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        if !self.will_wake(source) {
            *self = source.clone();
        }
    }
}

impl Drop for Waker {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: This is safe because `Waker::from_raw` is the only way
        // to initialize `drop` and `data` requiring the user to acknowledge
        // that the contract of `RawWaker` is upheld.
        unsafe { (self.waker.vtable.drop)(self.waker.data) }
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vtable_ptr = self.waker.vtable as *const RawWakerVTable;
        f.debug_struct("Waker")
            .field("data", &self.waker.data)
            .field("vtable", &vtable_ptr)
            .finish()
    }
}
