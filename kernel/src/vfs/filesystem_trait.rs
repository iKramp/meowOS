use core::fmt::Debug;
use std::{boxed::Box, error::KernelError, sync::arc::Arc};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::{DirEntry, MountedPartition},
    memory::addresses::PhysAddr,
    vfs::DeviceId,
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
    async fn unmount(&self) -> Result<(), KernelError>;
    ///Offset must be page aligned
    async fn read(&self, inode: InodeIndex, offset_bytes: u64, size_bytes: u64, buffer: &[PhysAddr]) -> Result<u64, KernelError>;
    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, KernelError>;
    ///Offset must be page aligned. Returns the new inode
    async fn write(&self, inode: InodeIndex, offset: u64, size: u64, buffer: &[PhysAddr]) -> Result<(Inode, u64), KernelError>;
    async fn stat(&self, inode: InodeIndex, parent: InodeIndex) -> Result<Inode, KernelError>;
    async fn set_stat(&self, inode_index: InodeIndex, parent: InodeIndex, inode_data: Inode) -> Result<(), KernelError>;
    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(
        &self,
        name: &str,
        parent_dir: InodeIndex,
        type_mode: InodeTypeAndPerms,
        uid: u16,
        gid: u16,
    ) -> Result<(Inode, Inode), KernelError>;
    //returns the new inodes (parent, child). Reaching link count 0 doesn't remove the file yet
    async fn unlink(&self, parent_inode: InodeIndex, name: &str) -> Result<(Inode, Inode), KernelError>;
    //removes the inode and all its data. Link count has to be 0
    async fn remove_inode(&self, inode: InodeIndex) -> Result<(), KernelError>;
    ///returns the new inodes (parent, child)
    async fn link(&self, inode: InodeIndex, parent_dir: InodeIndex, name: &str) -> Result<(Inode, Inode), KernelError>;
    async fn truncate(&self, inode: InodeIndex, size: u64) -> Result<(), KernelError>;
    async fn rename(&self, inode: InodeIndex, parent_inode: InodeIndex, name: &str) -> Result<(), KernelError>;
}
