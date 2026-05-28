use core::{
    any::{Any, TypeId},
    fmt::Debug,
};
use std::{error::ErrorCode, sync::arc::Arc, vec::Vec};

pub(in crate::proc) use filesystem_namespace::*;
pub(in crate::proc) use memory_namespace::*;
pub(in crate::proc) use syscall_namespace::*;

mod filesystem_namespace;
mod memory_namespace;
mod namespace_management_pack;
mod syscall_namespace;

pub(super) use namespace_management_pack::init_namespace_management_syscalls;

pub(super) trait ProcNamespace: Debug + Send + Sync + Any {
    fn get_id(&self) -> u64;
    fn init_from(&self, other: &Self) -> Result<(), ErrorCode>;
}

//update in documentation
#[repr(u32)]
pub(in crate::proc) enum NamespaceType {
    Syscall = 0,
    Mem = 1,
    Fs = 2,
}

#[derive(Debug)]
pub(in crate::proc) enum NamespaceHolder {
    Syscall(Arc<SyscallNamespace>),
    Mem(Arc<MemoryNamespace>),
    Fs(Arc<FilesystemNamespace>),
}

#[derive(Debug)]
pub(in crate::proc) struct ProcNamespaces {
    owned_namespaces: Vec<NamespaceHolder>,
    pub memory_namespace: Arc<MemoryNamespace>,
    syscall_namespace: Arc<SyscallNamespace>,
    filesystem_namespace: Arc<FilesystemNamespace>,
}

#[derive(Clone)]
#[repr(C)]
pub(in crate::proc) struct NamespaceIds {
    memory_namespace: u64,
    syscall_namespace: u64,
    filesystem_namespace: u64,
}

impl ProcNamespaces {
    pub fn new(
        memory_namespace: Arc<MemoryNamespace>,
        syscall_namespace: Arc<SyscallNamespace>,
        filesystem_namespace: Arc<FilesystemNamespace>,
    ) -> Self {
        let mut owned_namespaces = Vec::new();
        owned_namespaces.push(NamespaceHolder::Mem(memory_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::Syscall(syscall_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::Fs(filesystem_namespace.clone()));

        owned_namespaces.sort_by_key(|ns| ns.get_id());

        Self {
            owned_namespaces,
            memory_namespace,
            syscall_namespace,
            filesystem_namespace,
        }
    }

    pub fn clone_from_ids(&self, mut ids: NamespaceIds) -> Result<Self, ErrorCode> {
        //defaults
        if ids.memory_namespace == 0 {
            ids.memory_namespace = self.memory_namespace.get_id();
        }
        if ids.syscall_namespace == 0 {
            ids.syscall_namespace = self.syscall_namespace.get_id();
        }
        if ids.filesystem_namespace == 0 {
            ids.filesystem_namespace = self.filesystem_namespace.get_id();
        }

        let Some(NamespaceHolder::Mem(memory_namespace)) = self.get_namespace_holder(ids.memory_namespace) else {
            return Err(ErrorCode::InvalidArgument);
        };
        let Some(NamespaceHolder::Syscall(syscall_namespace)) = self.get_namespace_holder(ids.syscall_namespace) else {
            return Err(ErrorCode::InvalidArgument);
        };
        let Some(NamespaceHolder::Fs(filesystem_namespace)) = self.get_namespace_holder(ids.filesystem_namespace) else {
            return Err(ErrorCode::InvalidArgument);
        };
        Ok(Self::new(
            memory_namespace.clone(),
            syscall_namespace.clone(),
            filesystem_namespace.clone(),
        ))
    }

    pub fn get_namespace<T: ProcNamespace>(&self, id: u64) -> Option<Arc<T>> {
        if id == 0 {
            //default namespaces are always available
            Some(self.get_default_namespace::<T>())
        } else {
            self.get_indexed_namespace(id)
        }
    }

    fn get_indexed_namespace<T: ProcNamespace>(&self, id: u64) -> Option<Arc<T>> {
        let index = self.owned_namespaces.binary_search_by_key(&id, |ns| ns.get_id()).ok()?;
        let namespace = &self.owned_namespaces[index];
        namespace.try_unwrap()
    }

    fn get_default_namespace<T: ProcNamespace>(&self) -> Arc<T> {
        let default_id = match TypeId::of::<T>() {
            id if id == TypeId::of::<SyscallNamespace>() => self.syscall_namespace.get_id(),
            id if id == TypeId::of::<MemoryNamespace>() => self.memory_namespace.get_id(),
            _ => panic!("unsupported namespace type"),
        };
        self.get_indexed_namespace(default_id)
            .expect("default namespace should always be available")
    }

    pub fn change_namespace(&mut self, namespace_id: u64) -> Result<(), ()> {
        let index = self
            .owned_namespaces
            .binary_search_by_key(&namespace_id, |ns| ns.get_id())
            .map_err(|_| ())?;
        match &self.owned_namespaces[index] {
            NamespaceHolder::Syscall(ns) => self.syscall_namespace = ns.clone(),
            NamespaceHolder::Mem(ns) => self.memory_namespace = ns.clone(),
            NamespaceHolder::Fs(ns) => self.filesystem_namespace = ns.clone(),
        }
        Ok(())
    }

    pub fn add_namespace(&mut self, namespace: NamespaceHolder) {
        let id = match &namespace {
            NamespaceHolder::Syscall(ns) => ns.get_id(),
            NamespaceHolder::Mem(ns) => ns.get_id(),
            NamespaceHolder::Fs(ns) => ns.get_id(),
        };
        let index = self
            .owned_namespaces
            .binary_search_by_key(&id, |ns| ns.get_id())
            .unwrap_or_else(|e| e);
        self.owned_namespaces.insert(index, namespace);
    }

    pub fn get_namespace_holder(&self, namespace_id: u64) -> Option<&NamespaceHolder> {
        let index = self
            .owned_namespaces
            .binary_search_by_key(&namespace_id, |ns| ns.get_id())
            .ok()?;
        Some(&self.owned_namespaces[index])
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
            NamespaceHolder::Fs(ns) => ns.get_id(),
        }
    }

    pub fn get_type(&self) -> NamespaceType {
        match self {
            NamespaceHolder::Syscall(_) => NamespaceType::Syscall,
            NamespaceHolder::Mem(_) => NamespaceType::Mem,
            NamespaceHolder::Fs(_) => NamespaceType::Fs,
        }
    }

    pub fn init_from(&self, other: &Self) -> Result<(), ErrorCode> {
        match self {
            NamespaceHolder::Syscall(curr_ns) => {
                let other_ns = match other {
                    NamespaceHolder::Syscall(ns) => ns,
                    _ => return Err(ErrorCode::InvalidArgument),
                };
                curr_ns.init_from(other_ns)
            }
            NamespaceHolder::Mem(curr_ns) => {
                let other_ns = match other {
                    NamespaceHolder::Mem(ns) => ns,
                    _ => return Err(ErrorCode::InvalidArgument),
                };
                curr_ns.init_from(other_ns)
            }
            NamespaceHolder::Fs(curr_ns) => {
                let other_ns = match other {
                    NamespaceHolder::Fs(ns) => ns,
                    _ => return Err(ErrorCode::InvalidArgument),
                };
                curr_ns.init_from(other_ns)
            }
        }
    }

    pub fn try_unwrap<T: ProcNamespace>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let raw_ptr = match self {
            NamespaceHolder::Syscall(ns) if ns.type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Syscall(_) => return None,
            NamespaceHolder::Mem(ns) if ns.type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Mem(_) => return None,
            NamespaceHolder::Fs(ns) if ns.type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Fs(_) => return None,
            //no wildcard to get exhaustiveness checking
        };
        Some(unsafe { Arc::from_raw(raw_ptr) })
    }
}

impl NamespaceType {
    pub fn from_id(id: u64) -> Option<Self> {
        match id {
            0 => Some(Self::Syscall),
            1 => Some(Self::Mem),
            2 => Some(Self::Fs),
            _ => None,
        }
    }

    pub fn to_id(&self) -> u64 {
        match self {
            NamespaceType::Syscall => 0,
            NamespaceType::Mem => 1,
            NamespaceType::Fs => 2,
        }
    }

    pub fn create_empty_namespace(self, id: u64) -> NamespaceHolder {
        match self {
            NamespaceType::Syscall => NamespaceHolder::Syscall(Arc::new(SyscallNamespace::new(id))),
            NamespaceType::Mem => NamespaceHolder::Mem(Arc::new(MemoryNamespace::new(id))),
            NamespaceType::Fs => NamespaceHolder::Fs(Arc::new(FilesystemNamespace::new(id))),
        }
    }
}
