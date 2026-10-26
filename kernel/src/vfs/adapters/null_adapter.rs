use std::{
    boxed::Box,
    error::KernelError,
    kerror,
    sync::{arc::Arc, once_lock::OnceLock},
};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::DirEntry,
    memory::addresses::PhysAddr,
    vfs::{DeviceId, FileReadResult, FileSystem, Inode, adapters::VfsAdapterTrait},
};

#[derive(Debug)]
pub struct NullAdapter {
    device_id: crate::vfs::DeviceId,
    device_details: crate::vfs::DeviceDetails,
}

static NULL_ADAPTER: OnceLock<Arc<dyn FileSystem + Send>> = OnceLock::new();

impl NullAdapter {
    pub fn get() -> Arc<dyn FileSystem + Send> {
        NULL_ADAPTER
            .get_or_init(|| {
                let device_details =
                    crate::vfs::VFS_ADAPTER_DEVICE.allocate_device(&mut crate::vfs::lock_w_info!(crate::vfs::VFS));
                Arc::new(Self {
                    device_id: device_details.0,
                    device_details: device_details.1,
                })
            })
            .clone()
    }

    fn get_inode(&self, index: crate::vfs::InodeIndex) -> crate::vfs::Inode {
        crate::vfs::Inode {
            index,
            device: self.device_id,
            type_mode: crate::vfs::InodeTypeAndPerms::new_file(0o777),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: 0,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
            internal_synchronization: true,
        }
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for NullAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    fn partition_id(&self) -> Uuid {
        self.device_details.partition
    }

    fn open(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[u8]>, KernelError> {
        Ok(Box::new([]))
    }

    fn close(&self, _inode: crate::vfs::InodeIndex, _extra_data: Box<[u8]>) -> Result<(), KernelError> {
        Ok(())
    }

    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _extra_data: &Box<[u8]>,
        _offset_bytes: u64,
        _size_bytes: u64,
        _buffer: &[PhysAddr],
        _blocking: bool,
    ) -> Result<(u64, FileReadResult), KernelError> {
        Ok((0, FileReadResult::PermanentEof))
    }

    async fn read_dir(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, KernelError> {
        kerror!(UnsupportedOperation)
    }

    async fn write(
        &self,
        inode: crate::vfs::InodeIndex,
        _extra_data: &Box<[u8]>,
        _offset: u64,
        size: u64,
        _buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), KernelError> {
        Ok((self.get_inode(inode), size))
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<Inode, KernelError> {
        Ok(self.get_inode(inode))
    }
}
