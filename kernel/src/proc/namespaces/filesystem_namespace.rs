use core::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::btree_map::BTreeMap,
    error::KernelError,
    kerror, lock_w_info, r_lock_w_info,
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
pub struct FilesystemNamespace {
    id: u64,
    open_files: RWSpinlock<BTreeMap<u64, Arc<FileHandle>>>,
    file_handle_counter: AtomicU64,
    cwd: NoIntSpinlock<FileHandle>,
}

impl FilesystemNamespace {
    pub fn new(id: u64, cwd: FileHandle) -> Self {
        FilesystemNamespace {
            id,
            open_files: RWSpinlock::new(BTreeMap::new()),
            file_handle_counter: AtomicU64::new(1),
            cwd: NoIntSpinlock::new(cwd),
        }
    }

    pub fn open_file_handle(&self, file_handle: FileHandle) -> FileDescriptor {
        let mut internal = w_lock_w_info!(self.open_files);
        let fd = self.file_handle_counter.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        internal.insert(fd, Arc::new(file_handle));
        fd
    }

    pub fn get_inode(&self, fd: FileDescriptor) -> Option<InodeIdentifier> {
        let internal = r_lock_w_info!(self.open_files);
        internal.get(&fd).map(|handle| handle.inode)
    }

    pub fn get_parent_chain(&self, fd: FileDescriptor) -> Option<InodeIdentifierChain> {
        let internal = r_lock_w_info!(self.open_files);
        internal.get(&fd).map(|handle| handle.parent_chain.clone())
    }

    pub fn get_whole_chain(&self, fd: FileDescriptor) -> Option<InodeIdentifierChain> {
        let internal = r_lock_w_info!(self.open_files);
        let (chain, final_inode_index) = internal.get(&fd).map(|handle| (handle.parent_chain.clone(), handle.inode))?;
        let mut chain = chain.to_vec();
        chain.push(final_inode_index);
        Some(chain.into_boxed_slice())
    }

    pub fn get_file_handle(&self, fd: FileDescriptor) -> Option<Arc<FileHandle>> {
        let internal = r_lock_w_info!(self.open_files);
        internal.get(&fd).cloned()
    }

    pub fn close_file_handle(&self, fd: FileDescriptor) -> Option<Arc<FileHandle>> {
        let mut internal = w_lock_w_info!(self.open_files);
        internal.remove(&fd)
    }

    pub fn get_cwd_chain(&self) -> InodeIdentifierChain {
        let cwd = lock_w_info!(self.cwd);
        let chain = cwd.parent_chain.clone();
        let mut chain = chain.to_vec();
        chain.push(cwd.inode);
        chain.into_boxed_slice()
    }
}

impl ProcNamespace for FilesystemNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn create_empty(_id: u64) -> Result<Self, KernelError> {
        kerror!(InvalidOperation)
    }

    fn create_from(id: u64, other: &Self) -> Result<Self, KernelError> {
        let other_cwd = lock_w_info!(other.cwd);
        let other_open_files = r_lock_w_info!(other.open_files);
        let counter = other.file_handle_counter.load(Ordering::Relaxed);
        let mut open_files = BTreeMap::new();
        for (fd, handle) in other_open_files.iter() {
            open_files.insert(*fd, handle.clone());
        }
        let cwd = FileHandle::clone_from(&other_cwd);
        Ok(FilesystemNamespace {
            id,
            open_files: RWSpinlock::new(open_files),
            file_handle_counter: AtomicU64::new(counter),
            cwd: NoIntSpinlock::new(cwd),
        })
    }

    fn get_default(holder: &super::ProcNamespaces) -> Arc<Self> {
        holder.filesystem_namespace.clone()
    }
}
