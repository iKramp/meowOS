//!A module for a directory temporary filesystem (dtmpfs).
//!It is the root filesystem before any actual disk is mounted. It cannot store any files, but
//!provides a directory structure for mounpoints

use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    error::ErrorCode,
    lock_w_info,
    string::{String, ToString},
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use uuid::Uuid;

use crate::drivers::block_device::disk::DirEntry;

use super::{
    DeviceId, InodeIndex, InodeTypeAndPerms, ROOT_INODE_INDEX,
    filesystem_trait::{FileSystem, FileSystemFactory},
};

#[derive(Debug)]
pub(super) struct Dtmpfs {
    global_lock: NoIntSpinlock<DtmpfsInner>,
}

#[derive(Debug)]
struct DtmpfsInner {
    root: u64,
    inodes: BTreeMap<u64, DtmpfsNode>,
    inode_index: u64,
}

#[derive(Debug)]
struct DtmpfsNode {
    children: Vec<(String, u64)>, // (name, inode)
}

pub(super) fn init_dtmpfs() {
    crate::vfs::register_filesystem_driver_factory(Arc::new(DtmpfsFactory));
}

pub(super) struct DtmpfsFactory;

impl DtmpfsFactory {
    pub const UUID: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000000");
}

#[async_trait::async_trait]
impl FileSystemFactory for DtmpfsFactory {
    async fn mount(&self, _partition: crate::drivers::block_device::disk::MountedPartition) -> Arc<dyn FileSystem + Send> {
        let mut inodes = BTreeMap::new();
        inodes.insert(ROOT_INODE_INDEX, DtmpfsNode { children: Vec::new() });
        let fs = Dtmpfs {
            global_lock: NoIntSpinlock::new(DtmpfsInner {
                root: ROOT_INODE_INDEX, // Root inode index
                inodes,
                inode_index: 3,
            }),
        };
        Arc::new(fs)
    }

    fn uuid(&self) -> Uuid {
        DtmpfsFactory::UUID
    }

    fn name(&self) -> &str {
        "dtmpfs"
    }
}

#[async_trait::async_trait]
impl FileSystem for Dtmpfs {
    fn device_id(&self) -> DeviceId {
        unsafe { DeviceId(0) }
    }

    async fn unmount(&self) -> Result<(), ErrorCode> {
        Ok(())
    }

    async fn read(
        &self,
        _inode: InodeIndex,
        _offset_bytes: u64,
        _size_bytes: u64,
        _buffer: &[std::mem_utils::PhysAddr],
    ) -> Result<u64, ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode> {
        let inner = lock_w_info!(self.global_lock);
        let mut entries = Vec::new();
        if let Some(node) = inner.inodes.get(&inode) {
            for (name, child_inode) in &node.children {
                entries.push(crate::drivers::block_device::disk::DirEntry {
                    name: name.clone().into_boxed_str(),
                    inode: *child_inode,
                });
            }
        }
        drop(inner);
        Ok(entries.into_boxed_slice())
    }

    async fn write(
        &self,
        _inode: InodeIndex,
        _offset: u64,
        _size: u64,
        _buffer: &[std::mem_utils::PhysAddr],
    ) -> Result<(super::Inode, u64), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn stat(&self, inode: InodeIndex, _parent: InodeIndex) -> Result<super::Inode, ErrorCode> {
        Ok(super::Inode {
            index: inode,
            device: self.device_id(),
            type_mode: InodeTypeAndPerms::new_dir(0o755), //rwxr-xr-x
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: 0,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
        })
    }

    async fn set_stat(&self, _inode_index: InodeIndex, _parent: InodeIndex, _inode_data: super::Inode) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn create(
        &self,
        name: &str,
        parent_dir: InodeIndex,
        _type_mode: super::InodeTypeAndPerms,
        _uid: u16,
        _gid: u16,
    ) -> Result<(super::Inode, super::Inode), ErrorCode> {
        let mut inner = lock_w_info!(self.global_lock);
        let inode_index = inner.inode_index;
        inner.inode_index += 1;
        inner.inodes.insert(inode_index, DtmpfsNode { children: Vec::new() });

        let Some(parent_inode) = inner.inodes.get_mut(&parent_dir) else {
            return Err(ErrorCode::InodeNotPresent);
        };

        parent_inode.children.push((name.to_string(), inode_index));
        drop(inner);
        Ok((
            self.stat(parent_dir, parent_dir).await?,
            self.stat(inode_index, inode_index).await?,
        ))
    }

    async fn unlink(&self, _parent_inode: InodeIndex, _name: &str) -> Result<(super::Inode, super::Inode), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }

    async fn remove_inode(&self, _inode: InodeIndex) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn link(
        &self,
        _inode: InodeIndex,
        _parent_dir: InodeIndex,
        _name: &str,
    ) -> Result<(super::Inode, super::Inode), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn truncate(&self, _inode: InodeIndex, _size: u64) -> Result<(), ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn rename(&self, inode: InodeIndex, parent_inode: InodeIndex, name: &str) -> Result<(), ErrorCode> {
        let mut inner = lock_w_info!(self.global_lock);
        let Some(parent_node) = inner.inodes.get_mut(&parent_inode) else {
            return Ok(());
        };
        if let Some((_, child_inode)) = parent_node.children.iter_mut().find(|(n, _)| n == name) {
            *child_inode = inode;
        } else {
            parent_node.children.push((name.to_string(), inode));
        }
        Ok(())
    }
}
