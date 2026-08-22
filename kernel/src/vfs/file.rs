use core::{future::AsyncDrop, mem::ManuallyDrop, sync::atomic::AtomicU64};
use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    error::KernelError,
    ffi_future, kerror_unwrapped, lock_w_info,
    sync::{
        self,
        arc::Arc,
        async_rw_lock::{AsyncRWLockModeWrite, AsyncRWlock, AsyncRWlockGuard},
        no_int_spinlock::NoIntSpinlock,
    },
};

use bitfield::bitfield;

use crate::{
    task_runner::{self, PidOption},
    vfs::{Inode, VFS},
};

use super::{InodeIdentifier, InodeIdentifierChain};

pub type FileDescriptor = u64;

#[derive(Debug)]
struct FileStorage {
    open_files: BTreeMap<InodeIdentifier, sync::arc::Weak<OpenFile>>,
}

static FILE_STORAGE: NoIntSpinlock<FileStorage> = NoIntSpinlock::new(FileStorage {
    open_files: BTreeMap::new(),
});

#[derive(Debug)]
pub struct FileHandle {
    pub inode: InodeIdentifier,
    pub parent_chain: InodeIdentifierChain,
    pub position: AtomicU64,
    pub file_flags: FileFlags,
    pub(in crate::vfs) open_file: Arc<OpenFile>,
}

impl FileHandle {
    pub fn clone_from(other: &Self) -> Self {
        FileHandle {
            inode: other.inode,
            parent_chain: other.parent_chain.clone(),
            position: AtomicU64::new(other.position.load(core::sync::atomic::Ordering::SeqCst)),
            file_flags: other.file_flags,
            open_file: other.open_file.clone(),
        }
    }
}

#[derive(Debug)]
pub(in crate::vfs) struct OpenFile {
    pub inode: ManuallyDrop<Arc<AsyncRWlock<Inode>>>,
}

bitfield! {
    pub struct OpenFlags(u64);
    impl Debug;
    pub read, set_read: 0;
    pub write, set_write: 1;
    pub append, set_append: 2;
    pub truncate, set_truncate: 3;
}

bitfield! {
    #[derive(PartialEq, Eq)]
    pub struct FileFlags(u8);
    impl Debug;
    pub read, set_read: 0;
    pub write, set_write: 1;
    pub append, set_append: 2;
    pub dir, set_dir: 5;
}

impl FileFlags {
    pub const fn new() -> Self {
        FileFlags(0)
    }

    pub fn new_with_flags(read: bool, write: bool, append: bool, dir: bool) -> Self {
        let mut flags = FileFlags::new();
        if read {
            flags.set_read(true);
        }
        if write {
            flags.set_write(true);
        }
        if append {
            flags.set_append(true);
        }
        if dir {
            flags.set_dir(true);
        }
        flags
    }

    pub fn with_read(mut self, read: bool) -> Self {
        self.set_read(read);
        self
    }

    pub fn with_write(mut self, write: bool) -> Self {
        self.set_write(write);
        self
    }

    pub fn with_append(mut self, append: bool) -> Self {
        self.set_append(append);
        self
    }
}

async fn fill_inode_data(
    inode_id: InodeIdentifier,
    parent_id: InodeIdentifier,
    inode_lock: &mut AsyncRWlockGuard<'_, Inode, AsyncRWLockModeWrite>,
) -> Result<(), KernelError> {
    let vfs = lock_w_info!(VFS);
    let device = vfs.devices.get(&inode_id.device_id).ok_or(kerror_unwrapped!(NoEntry))?;
    let partition = vfs
        .mounted_filesystems
        .get(&device.partition)
        .ok_or(kerror_unwrapped!(NoEntry))?
        .clone();
    drop(vfs);
    let inode = partition.stat(inode_id.index, parent_id.index).await?;
    **inode_lock = inode;
    Ok(())
}

pub(in crate::vfs) async fn get_open_file(
    inode_id: InodeIdentifier,
    parent_id: InodeIdentifier,
) -> Result<Arc<OpenFile>, KernelError> {
    let mut file_storage = lock_w_info!(FILE_STORAGE);
    if let Some(open_file) = file_storage.open_files.get(&inode_id) {
        if let Some(open_file) = open_file.upgrade() {
            return Ok(open_file);
        }
        file_storage.open_files.remove(&inode_id);
    }

    let empty_inode = unsafe { Inode::empty() };
    let dummy_open_file = Arc::new(OpenFile {
        inode: ManuallyDrop::new(Arc::new(AsyncRWlock::new(empty_inode))),
    });
    let open_file_clone = dummy_open_file.clone();
    let mut inode_lock = open_file_clone.inode.lock_write().await; //instant because no other holders exist
    file_storage.open_files.insert(inode_id, Arc::downgrade(&dummy_open_file));

    drop(file_storage);

    fill_inode_data(inode_id, parent_id, &mut inode_lock).await?;

    Ok(dummy_open_file)
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        //anything as long as arc doesn't error. It's in ManuallyDrop
        let dummy_arc = unsafe { Arc::from_raw(0x1000 as *const AsyncRWlock<Inode>) };
        let mut dummy_manually_drop = ManuallyDrop::new(dummy_arc);

        core::mem::swap(&mut self.inode, &mut dummy_manually_drop);

        let dummy_open_file = OpenFile {
            inode: dummy_manually_drop, //now taken from self
        };

        let future = async move {
            //move to future
            let mut dummy_open_file = dummy_open_file;
            let dummy_open_file_ptr = &mut dummy_open_file;
            //we need to drop the open file asynchronously because the inode might have an async lock on it
            unsafe { core::future::async_drop_in_place(dummy_open_file_ptr).await };
            core::mem::forget(dummy_open_file); //don't retrigger drop
        };
        let future = Box::pin(future);
        let ffi_fut = ffi_future::future::into_ffi_future(future);
        task_runner::add_task(ffi_fut, PidOption::None);
    }
}

impl AsyncDrop for OpenFile {
    async fn drop(self: core::pin::Pin<&mut Self>) {
        unsafe { ManuallyDrop::drop(&mut self.get_mut().inode) };
    }
}
