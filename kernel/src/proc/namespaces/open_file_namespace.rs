use std::{
    alloc::vec::Vec,
    collections::btree_map::BTreeMap,
    error::KernelError,
    lock_w_info, r_lock_w_info,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock, rw_lock::RWSpinlock},
    w_lock_w_info,
};

use crate::{
    proc::ProcNamespace,
    vfs::{
        InodeIdentifier, InodeIdentifierChain,
        file::{FileDescriptor, FileHandle},
    },
};

#[derive(Debug)]
pub struct OpenFileNamespace {
    id: u64,
    open_files: RWSpinlock<OpenFileNamespaceOpenFiles>,
    cwd: NoIntSpinlock<FileHandle>,
}

#[derive(Debug)]
struct OpenFileNamespaceOpenFiles {
    open_files: BTreeMap<u64, Arc<FileHandle>>,
    file_handle_counter: u64,
}

impl OpenFileNamespace {
    pub fn new(id: u64, cwd: FileHandle) -> Self {
        OpenFileNamespace {
            id,
            open_files: RWSpinlock::new(OpenFileNamespaceOpenFiles {
                open_files: BTreeMap::new(),
                file_handle_counter: 0,
            }),
            cwd: NoIntSpinlock::new(cwd),
        }
    }

    pub fn open_file_handle(&self, file_handle: FileHandle, requested: Option<FileDescriptor>) -> FileDescriptor {
        let mut internal = w_lock_w_info!(self.open_files);
        'requested: {
            if let Some(fd) = requested {
                if internal.open_files.contains_key(&fd) {
                    break 'requested;
                }
                if fd >= internal.file_handle_counter {
                    internal.file_handle_counter = fd + 1;
                }
                internal.open_files.insert(fd, Arc::new(file_handle));
                return fd;
            }
        }
        let fd = internal.file_handle_counter;
        internal.file_handle_counter += 1;
        internal.open_files.insert(fd, Arc::new(file_handle));
        fd
    }

    pub fn get_inode(&self, fd: FileDescriptor) -> Option<InodeIdentifier> {
        let internal = r_lock_w_info!(self.open_files);
        internal.open_files.get(&fd).map(|handle| handle.inode)
    }

    pub fn get_parent_chain(&self, fd: FileDescriptor) -> Option<InodeIdentifierChain> {
        let internal = r_lock_w_info!(self.open_files);
        internal.open_files.get(&fd).map(|handle| handle.parent_chain.clone())
    }

    pub fn get_whole_chain(&self, fd: FileDescriptor) -> Option<InodeIdentifierChain> {
        let internal = r_lock_w_info!(self.open_files);
        let (chain, final_inode_index) = internal
            .open_files
            .get(&fd)
            .map(|handle| (handle.parent_chain.clone(), handle.inode))?;
        let mut chain = chain.to_vec();
        chain.push(final_inode_index);
        Some(chain.into_boxed_slice())
    }

    pub fn get_file_handle(&self, fd: FileDescriptor) -> Option<Arc<FileHandle>> {
        let internal = r_lock_w_info!(self.open_files);
        internal.open_files.get(&fd).cloned()
    }

    pub fn close_file_handle(&self, fd: FileDescriptor) -> Option<Arc<FileHandle>> {
        let mut internal = w_lock_w_info!(self.open_files);
        internal.open_files.remove(&fd)
    }

    pub fn get_cwd_chain(&self) -> InodeIdentifierChain {
        let cwd = lock_w_info!(self.cwd);
        let chain = cwd.parent_chain.clone();
        let mut chain = chain.to_vec();
        chain.push(cwd.inode);
        chain.into_boxed_slice()
    }

    pub fn get_root_chain(&self) -> InodeIdentifierChain {
        let cwd = lock_w_info!(self.cwd);
        if let Some(root) = cwd.parent_chain.first() {
            let mut chain = Vec::new();
            chain.push(*root);
            chain.into_boxed_slice()
        } else {
            let mut chain = Vec::new();
            chain.push(cwd.inode);
            chain.into_boxed_slice()
        }
    }
}

impl ProcNamespace for OpenFileNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn create_empty(_id: u64, other: &Self) -> Result<Self, KernelError> {
        let other_cwd = lock_w_info!(other.cwd);
        let cwd = FileHandle::clone_from(&other_cwd);
        Ok(OpenFileNamespace::new(_id, cwd))
    }

    fn create_from(id: u64, other: &Self) -> Result<Self, KernelError> {
        let other_cwd = lock_w_info!(other.cwd);
        let other_open_files = r_lock_w_info!(other.open_files);
        let counter = other_open_files.file_handle_counter;
        let mut open_files = BTreeMap::new();
        for (fd, handle) in other_open_files.open_files.iter() {
            open_files.insert(*fd, handle.clone());
        }
        let cwd = FileHandle::clone_from(&other_cwd);
        Ok(OpenFileNamespace {
            id,
            open_files: RWSpinlock::new(OpenFileNamespaceOpenFiles {
                open_files,
                file_handle_counter: counter,
            }),
            cwd: NoIntSpinlock::new(cwd),
        })
    }

    fn get_default(holder: &super::ProcNamespaces) -> Arc<Self> {
        holder.open_file_namespace.clone()
    }
}
