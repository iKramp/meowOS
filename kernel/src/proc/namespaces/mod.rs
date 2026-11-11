use core::{
    any::{Any, TypeId},
    fmt::Debug,
};
use std::{boxed::Box, error::KernelError, kerror, sync::arc::Arc, vec::Vec};

pub(in crate::proc) use memory_namespace::*;
pub use mount_namespace::*;
pub use open_file_namespace::*;
pub(in crate::proc) use syscall_namespace::*;
pub(in crate::proc) use time_namespace::*;

pub mod memory_namespace;
mod mount_namespace;
mod namespace_management_pack;
mod open_file_namespace;
mod syscall_namespace;
mod time_namespace;

pub(super) use namespace_management_pack::init_namespace_management_syscalls;

use crate::proc::ProcessData;

pub trait ProcNamespace: Debug + Send + Sync + Any + Sized {
    fn get_id(&self) -> u64;
    fn create_from(id: u64, other: &Self) -> Result<Self, KernelError>;
    fn create_empty(id: u64, other: &Self) -> Result<Self, KernelError>;
    fn get_default(holder: &ProcNamespaces) -> Arc<Self>;
}

//update in documentation
#[repr(u32)]
pub(in crate::proc) enum NamespaceType {
    Syscall = 0,
    Mem = 1,
    OpenFile = 2,
    Time = 3,
    Mount = 4,
}

#[derive(Debug, Clone)]
pub(in crate::proc) enum NamespaceHolder {
    Syscall(Arc<SyscallNamespace>),
    Mem(Arc<MemoryNamespace>),
    OpenFile(Arc<OpenFileNamespace>),
    Time(Arc<TimeNamespace>),
    Mount(Arc<MountNamespace>),
}

#[derive(Debug)]
pub struct ProcNamespaces {
    owned_namespaces: Vec<NamespaceHolder>,
    pub(in crate::proc) memory_namespace: Arc<MemoryNamespace>,
    syscall_namespace: Arc<SyscallNamespace>,
    open_file_namespace: Arc<OpenFileNamespace>,
    time_namespace: Arc<TimeNamespace>,
    mount_namespace: Arc<MountNamespace>,
}

#[derive(Clone)]
#[repr(C)]
pub(in crate::proc) struct NamespaceIds {
    memory_namespace: u64,
    syscall_namespace: u64,
    open_file_namespace: u64,
    time_namespace: u64,
    mount_namespace: u64,
}

type ChangeNamespaceFn = Box<dyn FnOnce(ProcessData)>;

impl ProcNamespaces {
    pub(in crate::proc) fn new(
        memory_namespace: Arc<MemoryNamespace>,
        syscall_namespace: Arc<SyscallNamespace>,
        open_file_namespace: Arc<OpenFileNamespace>,
        time_namespace: Arc<TimeNamespace>,
        mount_namespace: Arc<MountNamespace>,
    ) -> Self {
        let mut owned_namespaces = Vec::new();
        owned_namespaces.push(NamespaceHolder::Mem(memory_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::Syscall(syscall_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::OpenFile(open_file_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::Time(time_namespace.clone()));
        owned_namespaces.push(NamespaceHolder::Mount(mount_namespace.clone()));

        owned_namespaces.sort_by_key(|ns| ns.get_id());

        Self {
            owned_namespaces,
            memory_namespace,
            syscall_namespace,
            open_file_namespace,
            time_namespace,
            mount_namespace,
        }
    }

    pub(in crate::proc) fn clone_from_ids(&self, mut ids: NamespaceIds) -> Result<Self, KernelError> {
        //defaults
        if ids.memory_namespace == 0 {
            ids.memory_namespace = self.memory_namespace.get_id();
        }
        if ids.syscall_namespace == 0 {
            ids.syscall_namespace = self.syscall_namespace.get_id();
        }
        if ids.open_file_namespace == 0 {
            ids.open_file_namespace = self.open_file_namespace.get_id();
        }
        if ids.time_namespace == 0 {
            ids.time_namespace = self.time_namespace.get_id();
        }
        if ids.mount_namespace == 0 {
            ids.mount_namespace = self.mount_namespace.get_id();
        }

        let Some(NamespaceHolder::Mem(memory_namespace)) = self.get_namespace_holder(ids.memory_namespace) else {
            return kerror!(InvalidArgument);
        };
        let Some(NamespaceHolder::Syscall(syscall_namespace)) = self.get_namespace_holder(ids.syscall_namespace) else {
            return kerror!(InvalidArgument);
        };
        let Some(NamespaceHolder::OpenFile(open_file_namespace)) = self.get_namespace_holder(ids.open_file_namespace) else {
            return kerror!(InvalidArgument);
        };
        let Some(NamespaceHolder::Time(time_namespace)) = self.get_namespace_holder(ids.time_namespace) else {
            return kerror!(InvalidArgument);
        };
        let Some(NamespaceHolder::Mount(mount_namespace)) = self.get_namespace_holder(ids.mount_namespace) else {
            return kerror!(InvalidArgument);
        };
        Ok(Self::new(
            memory_namespace.clone(),
            syscall_namespace.clone(),
            open_file_namespace.clone(),
            time_namespace.clone(),
            mount_namespace.clone(),
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
        T::get_default(self)
    }

    fn get_default_by_type(&self, ns_type: NamespaceType) -> NamespaceHolder {
        match ns_type {
            NamespaceType::Syscall => NamespaceHolder::Syscall(SyscallNamespace::get_default(self)),
            NamespaceType::Mem => NamespaceHolder::Mem(MemoryNamespace::get_default(self)),
            NamespaceType::OpenFile => NamespaceHolder::OpenFile(OpenFileNamespace::get_default(self)),
            NamespaceType::Time => NamespaceHolder::Time(TimeNamespace::get_default(self)),
            NamespaceType::Mount => NamespaceHolder::Mount(MountNamespace::get_default(self)),
        }
    }

    pub fn change_namespace(&mut self, namespace_id: u64) -> Result<Option<ChangeNamespaceFn>, ()> {
        let index = self
            .owned_namespaces
            .binary_search_by_key(&namespace_id, |ns| ns.get_id())
            .map_err(|_| ())?;
        let fn_to_call = match &self.owned_namespaces[index] {
            NamespaceHolder::Syscall(ns) => {
                self.syscall_namespace = ns.clone();
                None
            }
            NamespaceHolder::Mem(ns) => {
                self.memory_namespace = ns.clone();
                let page_tree_root = ns.page_tree_root();
                let f: ChangeNamespaceFn = Box::new(move |proc_data: ProcessData| {
                    proc_data.set_page_tree(page_tree_root);
                });
                Some(f)
            }
            NamespaceHolder::OpenFile(ns) => {
                self.open_file_namespace = ns.clone();
                None
            }
            NamespaceHolder::Time(ns) => {
                self.time_namespace = ns.clone();
                None
            }
            NamespaceHolder::Mount(ns) => {
                self.mount_namespace = ns.clone();
                None
            }
        };
        Ok(fn_to_call)
    }

    pub(in crate::proc) fn add_namespace(&mut self, namespace: NamespaceHolder) {
        let id = match &namespace {
            NamespaceHolder::Syscall(ns) => ns.get_id(),
            NamespaceHolder::Mem(ns) => ns.get_id(),
            NamespaceHolder::OpenFile(ns) => ns.get_id(),
            NamespaceHolder::Time(ns) => ns.get_id(),
            NamespaceHolder::Mount(ns) => ns.get_id(),
        };
        let index = self
            .owned_namespaces
            .binary_search_by_key(&id, |ns| ns.get_id())
            .unwrap_or_else(|e| e);
        self.owned_namespaces.insert(index, namespace);
    }

    pub(in crate::proc) fn get_namespace_holder(&self, namespace_id: u64) -> Option<&NamespaceHolder> {
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
            NamespaceHolder::OpenFile(ns) => ns.get_id(),
            NamespaceHolder::Time(ns) => ns.get_id(),
            NamespaceHolder::Mount(ns) => ns.get_id(),
        }
    }

    pub fn get_type(&self) -> NamespaceType {
        match self {
            NamespaceHolder::Syscall(_) => NamespaceType::Syscall,
            NamespaceHolder::Mem(_) => NamespaceType::Mem,
            NamespaceHolder::OpenFile(_) => NamespaceType::OpenFile,
            NamespaceHolder::Time(_) => NamespaceType::Time,
            NamespaceHolder::Mount(_) => NamespaceType::Mount,
        }
    }

    pub fn create_from(id: u64, other: &Self) -> Result<Self, KernelError> {
        match other {
            NamespaceHolder::Syscall(other_ns) => {
                let Ok(new_ns) = SyscallNamespace::create_from(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Syscall(Arc::new(new_ns)))
            }
            NamespaceHolder::Mem(other_ns) => {
                let Ok(new_ns) = MemoryNamespace::create_from(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Mem(Arc::new(new_ns)))
            }
            NamespaceHolder::OpenFile(other_ns) => {
                let Ok(new_ns) = OpenFileNamespace::create_from(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::OpenFile(Arc::new(new_ns)))
            }
            NamespaceHolder::Time(other_ns) => {
                let Ok(new_ns) = TimeNamespace::create_from(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Time(Arc::new(new_ns)))
            }
            NamespaceHolder::Mount(other_ns) => {
                let Ok(new_ns) = MountNamespace::create_from(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Mount(Arc::new(new_ns)))
            }
        }
    }

    pub fn create_empty_from(id: u64, other: &Self) -> Result<Self, KernelError> {
        match other {
            NamespaceHolder::Syscall(other_ns) => {
                let Ok(new_ns) = SyscallNamespace::create_empty(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Syscall(Arc::new(new_ns)))
            }
            NamespaceHolder::Mem(other_ns) => {
                let Ok(new_ns) = MemoryNamespace::create_empty(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Mem(Arc::new(new_ns)))
            }
            NamespaceHolder::OpenFile(other_ns) => {
                let Ok(new_ns) = OpenFileNamespace::create_empty(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::OpenFile(Arc::new(new_ns)))
            }
            NamespaceHolder::Time(other_ns) => {
                let Ok(new_ns) = TimeNamespace::create_empty(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Time(Arc::new(new_ns)))
            }
            NamespaceHolder::Mount(other_ns) => {
                let Ok(new_ns) = MountNamespace::create_empty(id, other_ns) else {
                    return kerror!(InvalidArgument);
                };
                Ok(NamespaceHolder::Mount(Arc::new(new_ns)))
            }
        }
    }

    pub fn try_unwrap<T: ProcNamespace>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let raw_ptr = match self {
            NamespaceHolder::Syscall(ns) if ns.get().type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Syscall(_) => return None,
            NamespaceHolder::Mem(ns) if ns.get().type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Mem(_) => return None,
            NamespaceHolder::OpenFile(ns) if ns.get().type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::OpenFile(_) => return None,
            NamespaceHolder::Time(ns) if ns.get().type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Time(_) => return None,
            NamespaceHolder::Mount(ns) if ns.get().type_id() == type_id => Arc::into_raw(ns.clone()) as *const T,
            NamespaceHolder::Mount(_) => return None,
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
            2 => Some(Self::OpenFile),
            3 => Some(Self::Time),
            4 => Some(Self::Mount),
            _ => None,
        }
    }

    pub fn to_id(&self) -> u64 {
        match self {
            NamespaceType::Syscall => 0,
            NamespaceType::Mem => 1,
            NamespaceType::OpenFile => 2,
            NamespaceType::Time => 3,
            NamespaceType::Mount => 4,
        }
    }
}
