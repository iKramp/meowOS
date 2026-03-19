use alloc::collections::btree_set::BTreeSet;

use crate::mem_utils::get_heap_initialized;

static mut GLOBAL_LOCK_INFO: LockInfo = LockInfo::new();

/// fn to get lock info
/// # Safety
/// default function is only used for the BSP
pub(super) static mut GET_LOCK_INFO: fn() -> &'static mut LockInfo = || unsafe { &mut GLOBAL_LOCK_INFO };

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct LockLocationInfo(pub &'static str, pub u32, pub u32);

#[derive(Debug)]
pub struct LockInfo {
    num_no_int_spinlocks: u16,
    prev_int_state: bool,
    blocking_task: bool,
    #[cfg(debug_assertions)]
    locations: BTreeSet<LockLocationInfo>,
    #[cfg(debug_assertions)]
    memory_locked: u64,
}

impl LockInfo {
    pub const fn new() -> Self {
        Self {
            num_no_int_spinlocks: 0,
            prev_int_state: true,
            blocking_task: false,
            #[cfg(debug_assertions)]
            locations: BTreeSet::new(),
            #[cfg(debug_assertions)]
            memory_locked: 0,
        }
    }

    pub fn inc_spinlocks(&mut self, prev_int_state: bool, location: LockLocationInfo) {
        let prev_val = self.num_no_int_spinlocks;
        self.num_no_int_spinlocks = prev_val + 1;
        if prev_val == 0 {
            self.prev_int_state = prev_int_state;
        }
        #[cfg(debug_assertions)]
        {
            let is_mem_loc = location.0.contains("memory");
            if is_mem_loc {
                self.memory_locked += 1;
            }
            if self.memory_locked == 0 && get_heap_initialized() {
                self.locations.insert(location);
            }
        }
    }

    /// returns whether interrupts should be re-enabled
    pub fn dec_spinlocks(&mut self, location: &LockLocationInfo) -> bool {
        let old_val = self.num_no_int_spinlocks;
        self.num_no_int_spinlocks = old_val - 1;
        #[cfg(debug_assertions)]
        {
            let is_mem_loc = location.0.contains("memory");
            if is_mem_loc {
                self.memory_locked -= 1;
            }
            if get_heap_initialized() {
                self.locations.remove(location);
            }
        }

        if old_val == 1 { self.prev_int_state } else { false }
    }

    pub fn num_locks(&self) -> u16 {
        self.num_no_int_spinlocks
    }

    pub fn blocking_task(&mut self) {
        let prev_rflags: u64;
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) prev_rflags,
            );
        }
        let prev_int_state = (prev_rflags & (1 << 9)) != 0;
        self.blocking_task = true;
        if prev_int_state {
            unsafe {
                core::arch::asm!("sti");
            }
        }
    }

    pub fn unblocking_task(&mut self) {
        let prev_rflags: u64;
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) prev_rflags,
            );
        }
        let prev_int_state = (prev_rflags & (1 << 9)) != 0;
        self.blocking_task = false;
        if prev_int_state {
            unsafe {
                core::arch::asm!("sti");
            }
        }
    }

    pub fn is_blocking_task(&self) -> bool {
        self.blocking_task
    }

    pub fn assert_no_locks(&self) {
        if self.num_no_int_spinlocks == 0 {
            return;
        }
        unsafe { core::arch::asm!("cli") };
        #[cfg(debug_assertions)]
        panic!("Locks held: {:#?}", self.locations);
        panic!("{} locks held", self.num_no_int_spinlocks);
    }
}

impl Default for LockInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn set_lock_info_func(f: fn() -> &'static mut LockInfo) {
    assert!(unsafe { GLOBAL_LOCK_INFO.num_locks() == 0 });
    assert!(f().num_locks() == 0);
    unsafe {
        GET_LOCK_INFO = f;
    }
}
