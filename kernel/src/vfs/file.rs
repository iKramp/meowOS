use core::{future::AsyncDrop, sync::atomic::AtomicU64};
use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    error::ErrorCode,
    ffi_future, lock_w_info,
    sync::{self, arc::Arc, async_lock::AsyncSpinlock, no_int_spinlock::NoIntSpinlock},
};

use bitfield::bitfield;

use crate::{
    task_runner::{self, PidOption},
    vfs::{DeviceId, Inode, InodeTypeAndPerms, VFS},
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
    pub inode: AsyncSpinlock<Inode>,
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

async fn open_file(inode_id: InodeIdentifier) -> Result<Arc<OpenFile>, ErrorCode> {
    let vfs = lock_w_info!(VFS);
    let device = vfs.devices.get(&inode_id.device_id).ok_or(ErrorCode::NoEntry)?;
    let partition = vfs
        .mounted_filesystems
        .get(&device.partition)
        .ok_or(ErrorCode::NoEntry)?
        .clone();
    drop(vfs);
    let inode = partition.stat(inode_id.index).await?;

    let open_file = Arc::new(OpenFile {
        inode: AsyncSpinlock::new(inode),
    });
    lock_w_info!(FILE_STORAGE).open_files.insert(inode_id, open_file.downgrade());
    Ok(open_file)
}

pub(in crate::vfs) async fn get_file(inode_id: InodeIdentifier) -> Result<Arc<OpenFile>, ErrorCode> {
    let mut file_storage = lock_w_info!(FILE_STORAGE);
    if let Some(open_file) = file_storage.open_files.get(&inode_id) {
        if let Some(open_file) = open_file.upgrade() {
            return Ok(open_file);
        }
        file_storage.open_files.remove(&inode_id);
    }
    drop(file_storage);
    open_file(inode_id).await
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        let mut dummy_inode = Inode {
            index: 0,
            device: unsafe { DeviceId(0) },
            type_mode: InodeTypeAndPerms::new_file(0),
            link_cnt: 0,
            uid: 0,
            gid: 0,
            size: 0,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
        };

        let open_inode = unsafe { self.inode.get_read_ptr() as *const Inode };
        //safe because we're in drop (no other references)
        //due to safety (casting const to mut) open_inode should not be used again
        let open_inode_mut = open_inode as *mut Inode;

        unsafe { core::ptr::swap(open_inode_mut, &mut dummy_inode) }

        let dummy_open_file = OpenFile {
            inode: AsyncSpinlock::new(dummy_inode),
        };

        let future = async move {
            //move to future
            let dummy_open_file = dummy_open_file;
            let dummy_open_file_ptr = &dummy_open_file as *const OpenFile;
            //we need to drop the open file asynchronously because the inode might have an async lock on it
            unsafe { core::future::async_drop_in_place(dummy_open_file_ptr as *mut OpenFile).await };
            core::mem::forget(dummy_open_file);
        };
        let future = Box::pin(future);
        let ffi_fut = ffi_future::future::into_ffi_future(future);
        task_runner::add_task(ffi_fut, PidOption::None);
    }
}

impl AsyncDrop for OpenFile {
    async fn drop(self: core::pin::Pin<&mut Self>) {
        //First flush to disk, then clean up/invalidate any cached data
    }
}
