use core::sync::atomic::AtomicU64;
use std::{
    collections::btree_map::BTreeMap,
    error::ErrorCode,
    r_lock_w_info,
    sync::{arc::Arc, rw_lock::RWSpinlock},
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
}

impl FilesystemNamespace {
    pub fn new(id: u64) -> Self {
        FilesystemNamespace {
            id,
            open_files: RWSpinlock::new(BTreeMap::new()),
            file_handle_counter: AtomicU64::new(1),
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
}

impl ProcNamespace for FilesystemNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn init_from(&self, _other: &Self) -> Result<(), ErrorCode> {
        //For now, we just create a new empty filesystem namespace. In the future, we might want to share some state between namespaces.
        Ok(())
    }
}
