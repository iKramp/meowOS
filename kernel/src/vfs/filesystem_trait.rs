use core::fmt::Debug;
use std::{boxed::Box, error::KernelError, sync::arc::Arc};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::{DirEntry, MountedPartition},
    memory::addresses::PhysAddr,
    vfs::{
        DeviceId, FileReadResult, InodeExtraData,
        file::{EmptyFileHandleExtraData, FileHandleExtraData},
    },
};

use super::{Inode, InodeIndex, InodeTypeAndPerms};

#[async_trait::async_trait]
pub trait FileSystemFactory: Send + Sync {
    fn uuid(&self) -> Uuid;
    fn name(&self) -> &str;
    async fn mount(&self, partition: MountedPartition) -> Arc<dyn FileSystem + Send>;
}

#[async_trait::async_trait]
pub trait FileSystem: Debug + Send + Sync {
    fn device_id(&self) -> DeviceId;
    fn partition_id(&self) -> Uuid;
    ///Might be unused by some filesystems, but this is called for every new FileHandle, not Inode
    ///Filesystems may return extra data to be used later, but it can't be data that may become
    ///inconsistent by writes/reads to other instances of FileHandle to the same Inode
    ///open/close must be non-async becuase they are sometimes called from non-async contexts.
    ///If the filesystem needs to do async work, it should:
    ///on open - insert "pending" data and fill in real data on the next call
    ///on close - spawn a task to do the async work and return immediately
    fn f_handle_open(&self, _inode: InodeIndex) -> Result<Box<dyn FileHandleExtraData>, KernelError> {
        Ok(Box::new(EmptyFileHandleExtraData))
    }
    fn f_handle_close(&self, _inode: InodeIndex, _extra_data: Box<dyn FileHandleExtraData>) -> Result<(), KernelError> {
        Ok(())
    }

    async fn unmount(&self) -> Result<(), KernelError>;
    ///Offset must be page aligned
    async fn read(
        &self,
        inode: InodeIndex,
        f_handle_extra_data: &dyn FileHandleExtraData,
        inode_extra_data: &dyn InodeExtraData,
        offset_bytes: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
        blocking: bool,
    ) -> Result<(u64, FileReadResult), KernelError>;
    ///NOTE: The FS must handle EmptyExtraInodeData being passed in
    async fn read_dir(&self, inode: InodeIndex, inode_extra_data: &dyn InodeExtraData) -> Result<Box<[DirEntry]>, KernelError>;
    ///Offset must be page aligned. Returns the new inode
    async fn write(
        &self,
        inode: InodeIndex,
        f_handle_extra_data: &dyn FileHandleExtraData,
        inode_extra_data: &dyn InodeExtraData,
        offset: u64,
        size: u64,
        buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), KernelError>;
    ///NOTE: the value returned as inode extra data in stat must haeve internal synchronization if
    ///filesystem internal sync is also returned. IF filesystem internal sync is false,
    ///VFS will use a RW lock logic to protect the inode. In this case, the FS can use unsafe
    ///functions to update extra data on writes without internal sync (but not reads!!)
    async fn stat(&self, inode: InodeIndex, parent: InodeIndex) -> Result<Inode, KernelError>;
    async fn set_stat(
        &self,
        inode_index: InodeIndex,
        inode_data: Inode,
        inode_extra_data: &dyn InodeExtraData,
    ) -> Result<(), KernelError>;
    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(
        &self,
        name: &str,
        parent_dir: InodeIndex,
        parent_inode_extra_data: &dyn InodeExtraData,
        type_mode: InodeTypeAndPerms,
        uid: u16,
        gid: u16,
    ) -> Result<(Inode, Inode), KernelError>;
    //returns the new inodes (parent, child). Reaching link count 0 doesn't remove the file yet
    async fn unlink(
        &self,
        parent_inode: InodeIndex,
        parent_inode_extra_data: &dyn InodeExtraData,
        name: &str,
    ) -> Result<(Inode, Inode), KernelError>;
    //removes the inode and all its data. Link count has to be 0
    async fn remove_inode(&self, inode: InodeIndex) -> Result<(), KernelError>;
    ///returns the new inodes (parent, child)
    async fn link(
        &self,
        inode: InodeIndex,
        parent_dir: InodeIndex,
        parent_inode_extra_data: &dyn InodeExtraData,
        name: &str,
    ) -> Result<(Inode, Inode), KernelError>;
    async fn truncate(
        &self,
        inode: InodeIndex,
        f_handle_extra_data: &dyn FileHandleExtraData,
        inode_extra_data: &dyn InodeExtraData,
        size: u64,
    ) -> Result<(), KernelError>;
    async fn rename(
        &self,
        inode: InodeIndex,
        parent_inode: InodeIndex,
        parent_f_handle_extra_data: &dyn FileHandleExtraData,
        parent_inode_extra_data: &dyn InodeExtraData,
        name: &str,
    ) -> Result<(), KernelError>;
}
