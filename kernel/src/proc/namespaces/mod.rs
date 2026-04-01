use core::fmt::Debug;
use std::sync::{arc::Arc, no_int_spinlock::NoIntSpinlock};

pub(in crate::proc) use memory_namespace::*;

mod memory_namespace;

//not sync, each namespace has 1 owner process
trait ProcNamespace: Debug + Send {}

#[derive(Debug)]
pub(in crate::proc) struct ProcNamespaces {
    pub memory_namespace: Arc<NoIntSpinlock<MemoryNamespace>>,
}

impl ProcNamespaces {
    pub fn new(memory_namespace: Arc<NoIntSpinlock<MemoryNamespace>>) -> Self {
        Self { memory_namespace }
    }
}

impl Default for ProcNamespaces {
    fn default() -> Self {
        Self {
            memory_namespace: Arc::new(NoIntSpinlock::new(MemoryNamespace::default())),
        }
    }
}
