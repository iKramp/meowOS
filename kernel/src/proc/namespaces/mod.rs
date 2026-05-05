use core::fmt::Debug;
use std::{sync::arc::Arc, vec::Vec};

pub(in crate::proc) use memory_namespace::*;

pub(in crate::proc) use syscall_namespace::*;

mod memory_namespace;
mod namespace_management_syscalls;
mod syscall_namespace;

//not sync, each namespace has 1 owner process
trait ProcNamespace: Debug + Send + Sync {
    fn get_id(&self) -> u64;
}

//update in documentation
#[repr(u32)]
pub(in crate::proc) enum NamespaceType {
    Syscall = 0,
    Mem = 1,
}

#[derive(Debug)]
pub(in crate::proc) enum NamespaceHolder {
    Syscall(Arc<SyscallNamespace>),
    Mem(Arc<MemoryNamespace>),
}

#[derive(Debug)]
pub(in crate::proc) struct ProcNamespaces {
    owned_namespaces: Vec<NamespaceHolder>,
    pub memory_namespace: Arc<MemoryNamespace>,
    syscall_namespace: Arc<SyscallNamespace>,
}

impl ProcNamespaces {
    pub fn new(memory_namespace: Arc<MemoryNamespace>, syscall_namespace: Arc<SyscallNamespace>) -> Self {
        let mut owned_namespaces = Vec::new();
        owned_namespaces.push(NamespaceHolder::Mem(memory_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::Syscall(syscall_namespace.clone()));

        owned_namespaces.sort_by_key(|ns| match ns {
            NamespaceHolder::Syscall(ns) => ns.get_id(),
            NamespaceHolder::Mem(ns) => ns.get_id(),
        });

        Self {
            owned_namespaces,
            memory_namespace,
            syscall_namespace,
        }
    }

    pub fn get_syscall_namespace(&self) -> Arc<SyscallNamespace> {
        self.syscall_namespace.clone()
    }

    pub fn change_namespace(&mut self, namespace_id: u64) -> Result<(), ()> {
        let index = self
            .owned_namespaces
            .binary_search_by_key(&namespace_id, |ns| ns.get_id())
            .map_err(|_| ())?;
        match &self.owned_namespaces[index] {
            NamespaceHolder::Syscall(ns) => self.syscall_namespace = ns.clone(),
            NamespaceHolder::Mem(ns) => self.memory_namespace = ns.clone(),
        }
        Ok(())
    }

    pub fn add_namespace(&mut self, namespace: NamespaceHolder) {
        let id = match &namespace {
            NamespaceHolder::Syscall(ns) => ns.get_id(),
            NamespaceHolder::Mem(ns) => ns.get_id(),
        };
        let index = self
            .owned_namespaces
            .binary_search_by_key(&id, |ns| ns.get_id())
            .unwrap_or_else(|e| e);
        self.owned_namespaces.insert(index, namespace);
    }

    pub fn remove_namespace(&mut self, namespace_id: u64) -> Result<(), ()> {
        let index = self
            .owned_namespaces
            .binary_search_by_key(&namespace_id, |ns| ns.get_id())
            .map_err(|_| ())?;
        if self.is_in_use(namespace_id) {
            return Err(());
        }
        self.owned_namespaces.remove(index);
        Ok(())
    }

    pub fn is_in_use(&self, namespace_id: u64) -> bool {
        self.syscall_namespace.get_id() == namespace_id || self.memory_namespace.get_id() == namespace_id
    }
}

impl NamespaceHolder {
    pub fn get_id(&self) -> u64 {
        match self {
            NamespaceHolder::Syscall(ns) => ns.get_id(),
            NamespaceHolder::Mem(ns) => ns.get_id(),
        }
    }

    pub fn get_type(&self) -> NamespaceType {
        match self {
            NamespaceHolder::Syscall(_) => NamespaceType::Syscall,
            NamespaceHolder::Mem(_) => NamespaceType::Mem,
        }
    }
}

impl NamespaceType {
    pub fn from_id(id: u64) -> Option<Self> {
        match id {
            0 => Some(Self::Syscall),
            1 => Some(Self::Mem),
            _ => None,
        }
    }

    pub fn to_id(&self) -> u64 {
        match self {
            NamespaceType::Syscall => 0,
            NamespaceType::Mem => 1,
        }
    }

    pub fn create_empty_namespace(self, id: u64) -> NamespaceHolder {
        match self {
            NamespaceType::Syscall => NamespaceHolder::Syscall(Arc::new(SyscallNamespace::new(id))),
            NamespaceType::Mem => NamespaceHolder::Mem(Arc::new(MemoryNamespace::new(id))),
        }
    }
}
