use core::fmt::Debug;
use std::sync::arc::Arc;

pub(in crate::proc) use memory_namespace::*;

mod memory_namespace;

//not sync, each namespace has 1 owner process
trait ProcNamespace: Debug + Send {}

pub(in crate::proc) struct ProcNamespaces {
    memory_namespace: Arc<MemoryNamespace>,
}
