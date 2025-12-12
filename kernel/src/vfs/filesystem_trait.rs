use core::fmt::Debug;
use std::{boxed::Box, error::ErrorCode, mem_utils::PhysAddr, sync::arc::Arc};

use crate::drivers::disk::{DirEntry, MountedPartition};

use super::{Inode, InodeIndex, InodeType};


#[async_trait::async_trait]
pub trait FileSystemFactory: Send + Sync {
    async fn mount(&self, partition: MountedPartition) -> Arc<dyn FileSystem + Send>;
}

#[async_trait::async_trait]
pub trait FileSystem: Debug + Send + Sync {
    async fn unmount(&self);
    ///Offset must be page aligned
    async fn read(&self, inode: InodeIndex, offset_bytes: u64, size_bytes: u64, buffer: &[PhysAddr]) -> u64;
    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode>;
    ///Offset must be page aligned. Returns the new inode
    async fn write(&self, inode: InodeIndex, offset: u64, size: u64, buffer: &[PhysAddr]) -> (Inode, u64);
    async fn stat(&self, inode: InodeIndex) -> Inode;
    async fn set_stat(&self, inode_index: InodeIndex, inode_data: Inode);
    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(&self, name: &str, parent_dir: InodeIndex, type_mode: InodeType, uid: u16, gid: u16) -> (Inode, Inode);
    async fn unlink(&self, parent_inode: InodeIndex, name: &str);
    ///returns the new parent inode
    async fn link(&self, inode: InodeIndex, parent_dir: InodeIndex, name: &str) -> Inode;
    async fn truncate(&self, inode: InodeIndex, size: u64);
    async fn rename(&self, inode: InodeIndex, parent_inode: InodeIndex, name: &str) -> Result<(), ErrorCode>;
}
