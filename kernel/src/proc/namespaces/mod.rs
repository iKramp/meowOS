use core::fmt::Debug;
use std::sync::{arc::Arc, no_int_spinlock::NoIntSpinlock};

pub(in crate::proc) use memory_namespace::*;

pub(in crate::proc) use syscall_namespace::*;

mod memory_namespace;
mod syscall_namespace;

//not sync, each namespace has 1 owner process
trait ProcNamespace: Debug + Send {}

#[derive(Debug)]
pub(in crate::proc) struct ProcNamespaces {
    pub memory_namespace: Arc<NoIntSpinlock<MemoryNamespace>>,
    syscall_namespace: Arc<SyscallNamespace>,
}

impl ProcNamespaces {
    pub fn new(memory_namespace: Arc<NoIntSpinlock<MemoryNamespace>>, syscall_namespace: Arc<SyscallNamespace>) -> Self {
        Self {
            memory_namespace,
            syscall_namespace,
        }
    }

    pub fn get_syscall_namespace(&self) -> Arc<SyscallNamespace> {
        self.syscall_namespace.clone()
    }
}
