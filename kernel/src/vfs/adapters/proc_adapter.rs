use crate::vfs::{DeviceId, InodeType};

use super::{DirEntry, VfsAdapterTrait};
use std::{boxed::Box, error::ErrorCode};

#[derive(Debug)]
pub struct ProcAdapter {
    device_id: crate::vfs::DeviceId,
}

impl ProcAdapter {
    pub fn new(device_id: crate::vfs::DeviceId) -> Self {
        ProcAdapter { device_id }
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for ProcAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset_bytes: u64,
        _size_bytes: u64,
        _buffer: &[std::mem_utils::PhysAddr],
    ) -> Result<u64, ErrorCode> {
        todo!()
    }

    async fn read_dir(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode> {
        todo!()
    }

    async fn write(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset: u64,
        _size: u64,
        _buffer: &[std::mem_utils::PhysAddr],
    ) -> Result<(crate::vfs::Inode, u64), ErrorCode> {
        todo!()
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<crate::vfs::Inode, ErrorCode> {
        Ok(crate::vfs::Inode {
            index: inode,
            device: self.device_id,
            type_mode: InodeType::new_dir(0o755),
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
