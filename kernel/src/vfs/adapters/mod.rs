use core::{fmt::Debug, sync::atomic::AtomicU32};
use std::{boxed::Box, error::ErrorCode, mem_utils::PhysAddr};

use crate::drivers::block_device::disk::DirEntry;

use super::{filesystem_trait::FileSystem, DeviceDetails, DeviceId, Inode, InodeIndex, Vfs};

mod proc_adapter;
mod tty_adapter;
mod net_adapter;

pub use proc_adapter::ProcAdapter;
pub use tty_adapter::TtyAdapter;
use uuid::Uuid;

#[derive(Debug)]
pub(super) struct VfsAdapterDevice {
    part_count: AtomicU32,
}

impl VfsAdapterDevice {
    pub const fn new() -> Self {
        Self {
            part_count: AtomicU32::new(0),
        }
    }

    fn get_partition(&self) -> u64 {
        self.part_count.fetch_add(1, core::sync::atomic::Ordering::SeqCst) as u64
    }

    pub fn allocate_device(&self, vfs: &mut Vfs) -> (DeviceId, DeviceDetails) {
        let new_part = self.get_partition();
        let device = vfs.allocate_device();
        let dev_details = DeviceDetails {
            drive: Uuid::nil(),
            partition: Uuid::from_u64_pair(0, new_part),
        };
        vfs.devices.insert(device, dev_details.clone());
        (device, dev_details)
    }
}

#[async_trait::async_trait]
pub trait VfsAdapterTrait: Debug + Send + Sync {
    async fn read(&self, inode: InodeIndex, offset_bytes: u64, size_bytes: u64, buffer: &[PhysAddr]) -> Result<u64, ErrorCode>;
    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode>;
    async fn write(&self, inode: InodeIndex, offset: u64, size: u64, buffer: &[PhysAddr]) -> Result<(Inode, u64), ErrorCode>;
    async fn stat(&self, inode: InodeIndex) -> Result<Inode, ErrorCode>;
}

#[async_trait::async_trait]
impl<T: VfsAdapterTrait> FileSystem for T {
    async fn read(&self, inode: InodeIndex, offset_bytes: u64, size_bytes: u64, buffer: &[PhysAddr]) -> Result<u64, ErrorCode> {
        VfsAdapterTrait::read(self, inode, offset_bytes, size_bytes, buffer).await
    }

    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode> {
        VfsAdapterTrait::read_dir(self, inode).await
    }

    async fn write(&self, inode: InodeIndex, offset: u64, size: u64, buffer: &[PhysAddr]) -> Result<(Inode, u64), ErrorCode> {
        VfsAdapterTrait::write(self, inode, offset, size, buffer).await
    }

    async fn stat(&self, inode: InodeIndex) -> Result<Inode, ErrorCode> {
        VfsAdapterTrait::stat(self, inode).await
    }

    async fn unmount(&self) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn set_stat(&self, _inode_index: InodeIndex, _inode_data: Inode) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn create(
        &self,
        _name: &str,
        _parent_dir: InodeIndex,
        _type_mode: super::InodeType,
        _uid: u16,
        _gid: u16,
    ) -> Result<(Inode, Inode), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn unlink(&self, _parent_inode: InodeIndex, _name: &str) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn link(&self, _inode: InodeIndex, _parent_dir: InodeIndex, _name: &str) -> Result<Inode, ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn truncate(&self, _inode: InodeIndex, _size: u64) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn rename(&self, _inode: InodeIndex, _parent_inode: InodeIndex, _name: &str) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }
}
