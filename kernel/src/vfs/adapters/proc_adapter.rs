use crate::{
    memory::addresses::*,
    vfs::{DeviceId, FileSystem, InodeTypeAndPerms},
};

use super::{DirEntry, VfsAdapterTrait};
use std::{
    boxed::Box,
    error::KernelError,
    lock_w_info, println,
    sync::{arc::Arc, once_lock::OnceLock},
};

static PROC_ADAPTER: OnceLock<Arc<dyn FileSystem + Send>> = OnceLock::new();

#[derive(Debug)]
pub struct ProcAdapter {
    device_id: crate::vfs::DeviceId,
    device_details: crate::vfs::DeviceDetails,
}

impl ProcAdapter {
    pub fn get() -> Arc<dyn FileSystem + Send> {
        PROC_ADAPTER
            .get_or_init(|| {
                let device_details = crate::vfs::VFS_ADAPTER_DEVICE.allocate_device(&mut lock_w_info!(crate::vfs::VFS));
                println!("proc adapter created with device_id: {:?}", device_details.0);
                Arc::new(Self {
                    device_id: device_details.0,
                    device_details: device_details.1,
                })
            })
            .clone()
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for ProcAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    fn partition_id(&self) -> uuid::Uuid {
        self.device_details.partition
    }

    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset_bytes: u64,
        _size_bytes: u64,
        _buffer: &[PhysAddr],
    ) -> Result<u64, KernelError> {
        todo!()
    }

    async fn read_dir(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, KernelError> {
        todo!()
    }

    async fn write(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset: u64,
        _size: u64,
        _buffer: &[PhysAddr],
    ) -> Result<(crate::vfs::Inode, u64), KernelError> {
        todo!()
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<crate::vfs::Inode, KernelError> {
        Ok(crate::vfs::Inode {
            index: inode,
            device: self.device_id,
            type_mode: InodeTypeAndPerms::new_dir(0o755),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: 0,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
        })
    }
}
