use std::{
    boxed::Box,
    lock_w_info,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use crate::proc::{ProcessData, syscall::SyscallCpuState};

pub type SyscallHandler = fn(&SyscallCpuState, &Arc<ProcessData>) -> bool;

#[derive(Debug)]
pub struct SyscallPack {
    handlers: Box<[SyscallHandler]>,
}

type SyscallPackEntry = (Box<str>, Arc<SyscallPack>);

static SYSCALL_REGISTRY: SyscallRegistry = SyscallRegistry::new();

struct SyscallRegistry {
    registered_packs: NoIntSpinlock<Vec<SyscallPackEntry>>,
}

impl SyscallPack {
    pub fn new(handlers: Box<[SyscallHandler]>) -> Self {
        Self { handlers }
    }

    pub fn get_handler(&self, index: usize) -> Option<SyscallHandler> {
        self.handlers.get(index).copied()
    }

    pub fn num_syscalls(&self) -> usize {
        self.handlers.len()
    }
}

impl SyscallRegistry {
    pub const fn new() -> Self {
        Self {
            registered_packs: NoIntSpinlock::new(Vec::new()),
        }
    }

    pub fn register_syscall_pack(&self, name: Box<str>, pack: Arc<SyscallPack>) {
        let mut packs = lock_w_info!(self.registered_packs);
        packs.push((name, pack));
    }

    pub fn get_syscall_pack(&self, name: &str) -> Option<Arc<SyscallPack>> {
        let packs = lock_w_info!(self.registered_packs);
        for (pack_name, pack) in packs.iter() {
            if pack_name.as_ref() == name {
                return Some(pack.clone());
            }
        }
        None
    }
}

pub fn register_syscall_pack(name: Box<str>, pack: Arc<SyscallPack>) {
    assert!(pack.handlers.len() <= 32, "SyscallPack can have at most 32 handlers");
    SYSCALL_REGISTRY.register_syscall_pack(name, pack);
}

pub fn get_syscall_pack(name: &str) -> Option<Arc<SyscallPack>> {
    SYSCALL_REGISTRY.get_syscall_pack(name)
}
