use core::{cell::UnsafeCell, fmt::Debug, ops::{Deref, DerefMut}};

use crate::rc::Rc;


#[derive(Clone)]
pub struct Cow<T: Clone> {
    data: Rc<UnsafeCell<T>>
}

impl<T: Clone> Cow<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: Rc::new(UnsafeCell::new(data))
        }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.data.get().get() }
    }

    pub fn get_mut(&mut self) -> &mut T {
        if self.data.ref_count() == 1 {
            unsafe { &mut *self.data.get().get() }
        } else {
            self.data = Rc::new(UnsafeCell::new(self.get().clone()));
            unsafe { &mut *self.data.get().get() }
        }
    }
}

impl<T: Clone> DerefMut for Cow<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T: Clone> Deref for Cow<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: Clone + Debug> Debug for Cow<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cow").field("data", &self.data).finish()
    }
}
