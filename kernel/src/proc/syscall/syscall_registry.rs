use core::sync::atomic::AtomicU64;
use std::{
    boxed::Box,
    format, lock_w_info,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use crate::proc::{ProcessData, syscall::SyscallCpuState};

pub type SyscallHandler = fn(&SyscallCpuState, &Arc<ProcessData>) -> bool;

#[derive(Debug)]
pub struct SyscallPack {
    handlers: Box<[SyscallHandler]>,
}

type SyscallPackEntry = (Box<str>, u64, Arc<SyscallPack>);

static SYSCALL_REGISTRY: SyscallRegistry = SyscallRegistry::new();
static SYSCALL_PACK_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

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

    fn register_syscall_pack(&self, name: Box<str>, pack: Arc<SyscallPack>) {
        let mut packs = lock_w_info!(self.registered_packs);
        let id = SYSCALL_PACK_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        packs.push((name, id, pack));
    }

    fn get_syscall_pack(&self, name: &str) -> Option<(Arc<SyscallPack>, u64)> {
        let packs = lock_w_info!(self.registered_packs);
        for (pack_name, pack_id, pack) in packs.iter() {
            if pack_name.as_ref() == name {
                return Some((pack.clone(), *pack_id));
            }
        }
        None
    }
}

pub fn register_syscall_pack(name: Box<str>, pack: Arc<SyscallPack>) {
    assert!(pack.handlers.len() < 32, "SyscallPack can have at most 31 handlers");
    SYSCALL_REGISTRY.register_syscall_pack(name, pack);
}

pub fn get_syscall_pack(name: &str) -> Option<(Arc<SyscallPack>, u64)> {
    SYSCALL_REGISTRY.get_syscall_pack(name)
}

pub fn get_names(ids: impl Iterator<Item = u64>) -> Vec<Box<str>> {
    let packs = lock_w_info!(SYSCALL_REGISTRY.registered_packs);
    let mut names = Vec::new();
    'id_loop: for id in ids {
        for (pack_name, pack_id, _) in packs.iter() {
            if *pack_id == id {
                names.push(pack_name.clone());
                continue 'id_loop;
            }
        }
        names.push(format!("unknown({})", id).into_boxed_str());
    }
    names
}

pub fn get_all_pack_info() -> Vec<(Box<str>, u8)> {
    let packs = lock_w_info!(SYSCALL_REGISTRY.registered_packs);
    packs
        .iter()
        .map(|(name, _, syscalls)| (name.clone(), syscalls.handlers.len() as u8))
        .collect()
}
