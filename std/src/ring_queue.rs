use core::{
    cell::UnsafeCell,
    mem::{ManuallyDrop, MaybeUninit},
    sync::atomic::{
        AtomicU64,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use alloc::boxed::Box;
use bitfield::bitfield;

bitfield! {
    struct QueueState(u64);
    advance_head, set_advance_head: 15, 0;
    head, set_head: 31, 16;
    tail, set_tail: 47, 32;
}

// Head is the first non-allocated data
// Tail is the first allocated data
// Allocated: tail..head
// Free: head..tail
pub struct RingQueue<T> {
    buffer: Box<[ManuallyDrop<UnsafeCell<T>>]>,
    state: AtomicU64,
    capacity: u16,
}

unsafe impl<T> Sync for RingQueue<T> {}
unsafe impl<T> Send for RingQueue<T> {}

impl<T> RingQueue<T> {
    pub fn new(capacity: u16) -> Self {
        debug_assert!(capacity > 1);

        let data = Box::new_uninit_slice(capacity as usize);

        Self {
            buffer: unsafe { data.assume_init() },
            state: AtomicU64::new(0),
            capacity,
        }
    }

    //returns true if data went in
    pub fn push(&self, data: T) -> bool {
        let mut state = QueueState(self.state.load(Relaxed));

        let advance_head = loop {
            let next = (state.advance_head() + 1) % self.capacity as u64;
            if next == state.tail() {
                return false;
            }
            let new_state = {
                let mut a = state;
                a.set_advance_head(next);
                a
            };
            let prev = self.state.compare_exchange(state.0, new_state.0, Relaxed, Relaxed);
            match prev {
                Ok(_) => break new_state.0,
                Err(prev) => state = QueueState(prev),
            }
        };

        let data_ptr = self.buffer[advance_head as usize].get();
        unsafe { data_ptr.write(data) };

        let mut state = QueueState(self.state.load(Relaxed));
        loop {
            let previous = (advance_head + self.capacity as u64 - 1) % self.capacity as u64;
            let expected = {
                let mut a = state;
                a.set_head(previous);
                a
            };
            let new = {
                let mut a = state;
                a.set_head(advance_head);
                a
            };
            let prev = self.state.compare_exchange(expected.0, new.0, Release, Relaxed);
            match prev {
                Ok(_) => return true,
                Err(prev) => state = QueueState(prev),
            };
        }
    }

    pub fn get_first(&self) -> Option<T> {
        let mut data_container: MaybeUninit<T> = MaybeUninit::uninit();

        let mut state = QueueState(self.state.load(Acquire));

        'outer: loop {
            if state.head() == state.tail() {
                return None;
            }

            let data_ptr = data_container.as_mut_ptr();
            let src_ptr = self.buffer[state.tail() as usize].get();
            unsafe { data_ptr.copy_from(src_ptr, 1) };

            let mut inner_state = state;
            let current_tail = state.tail();
            let next_tail = (current_tail + 1) % self.capacity as u64;
            'inner: loop {
                let new_state = {
                    let mut a = inner_state;
                    a.set_tail(next_tail);
                    a
                };
                let res = self.state.compare_exchange(inner_state.0, new_state.0, Relaxed, Relaxed);
                match res {
                    Ok(_) => return unsafe { Some(data_container.assume_init()) },
                    Err(new_state) => {
                        let new_state = QueueState(new_state);
                        if new_state.tail() == current_tail {
                            inner_state = new_state;
                            continue 'inner;
                        } else {
                            state = new_state;
                            continue 'outer;
                        }
                    }
                }
            }
        }
    }

    pub fn append(&self, other: RingQueue<T>) {
        let mut other_state = QueueState(other.state.load(Relaxed));

        debug_assert!(other_state.advance_head() == other_state.head());
        for i in other_state.tail()..other_state.head() {
            let other_ptr = other.buffer[i as usize].get();
            let res = self.push(unsafe { other_ptr.read() });
            if !res {
                //can't accept more data. Drop the rest
                //i was already dropped
                other_state.set_tail(i + 1);
                other.state.store(other_state.0, Relaxed);
                //other will be dropped
                return;
            }
        }
        other_state.set_tail(other_state.head());
        other.state.store(other_state.0, Relaxed);
        //other will be dropped
    }

    pub fn append_iter(&self, iter: impl IntoIterator<Item = T>) {
        for item in iter {
            let res = self.push(item);
            if !res {
                return;
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        let state = QueueState(self.state.load(Relaxed));
        state.head() == state.tail()
    }
}

impl<T> Drop for RingQueue<T> {
    fn drop(&mut self) {
        let state = QueueState(self.state.load(Relaxed));
        debug_assert!(state.advance_head() == state.head());
        for i in state.tail()..state.head() {
            let elem = &mut self.buffer[i as usize];
            unsafe { ManuallyDrop::drop(elem) };
        }
    }
}
