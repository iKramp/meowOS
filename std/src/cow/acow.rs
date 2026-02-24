use core::{cell::UnsafeCell, fmt::Debug, ops::{Deref, DerefMut}};

use crate::sync::arc::Arc;

/// Async Cow version
#[derive(Clone)]
pub struct Acow<T: Clone> {
    data: Arc<UnsafeCell<T>>
}

unsafe impl<T: Send + Sync + Clone> Send for Acow<T> {}
unsafe impl<T: Send + Sync + Clone> Sync for Acow<T> {}

impl<T: Clone> Acow<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: Arc::new(UnsafeCell::new(data))
        }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.data.get().get() }
    }

    pub fn get_mut(&mut self) -> &mut T {
        if self.data.ref_count() == 1 {
            unsafe { &mut *self.data.get().get() }
        } else {
            self.data = Arc::new(UnsafeCell::new(self.get().clone()));
            unsafe { &mut *self.data.get().get() }
        }
    }
}

impl<T: Clone> DerefMut for Acow<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T: Clone> Deref for Acow<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: Clone + Debug> Debug for Acow<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cow").field("data", &self.data).finish()
    }
}
