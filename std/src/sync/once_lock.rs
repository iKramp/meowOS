use core::cell::Cell;



#[derive(Debug, Default)]
pub struct OnceLock<T> {
    value: core::cell::UnsafeCell<Option<T>>,
    is_initialized: Cell<bool>,
    initializing: core::sync::atomic::AtomicBool,
}

unsafe impl <T: Send> Send for OnceLock<T> {}
unsafe impl <T: Send + Sync> Sync for OnceLock<T> {}

impl<T> OnceLock<T> {
    pub const fn new() -> Self {
        Self {
            value: core::cell::UnsafeCell::new(None),
            is_initialized: Cell::new(false),
            initializing: core::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.is_initialized.get() {
            // Safe because once is_initialized is true, the value is never modified again
            unsafe { (*self.value.get()).as_ref() }
        } else {
            None
        }
    }

    pub fn get_or_init<F>(&self, init: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if !self.is_initialized.get() {
            // Try to acquire the initialization lock
            while self
                .initializing
                .compare_exchange(false, true, core::sync::atomic::Ordering::Acquire, core::sync::atomic::Ordering::Relaxed)
                .is_err()
            {
                // Spin-wait
                core::hint::spin_loop();
            }

            // Double-check if initialized after acquiring the lock
            if !self.is_initialized.get() {
                let value = init();
                unsafe {
                    *self.value.get() = Some(value);
                }
                self.is_initialized.set(true);
            }

            // Release the initialization lock
            self.initializing.store(false, core::sync::atomic::Ordering::Release);
        }

        // Safe because once is_initialized is true, the value is never modified again
        unsafe { (*self.value.get()).as_ref().unwrap_unchecked() }
    }
}
