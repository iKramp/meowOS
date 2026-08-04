use crate::memory::addresses::*;
use core::str::{self};
use std::{boxed::Box, error::ErrorCode, println, string::ToString, vec::Vec};

use super::InodeIndex;
use crate::vfs::InodeIndex as VfsInodeIndex;
use crate::vfs::{DeviceId, Inode as VfsInode};
use crate::{
    drivers::{
        block_device::disk::DirEntry as VfsDirEntry,
        filesystem::rfs2::{BLOCK_SIZE_SECTORS, BlockPtr, Rfs2, WorkingBlock, btree::BTreeNode},
    },
    memory::physical_allocator,
    vfs::{FileSystem, InodeTypeAndPerms},
};

mod dir_ops;
mod format;
mod increase_size;
mod read;
mod truncate;
mod write;

const PTRS_PER_BLOCK: usize = 4096 / core::mem::size_of::<BlockPtr>();
const PTRS_IN_ROOT: usize = (BLOCK_SIZE_SECTORS - 1) * 512 / core::mem::size_of::<BlockPtr>();

#[repr(C)]
#[derive(Debug, Clone)]
struct DirEntry {
    inode: InodeIndex,
    len: u8,
    name: [u8; 256 - core::mem::size_of::<InodeIndex>() - 1],
}

impl DirEntry {
    pub fn is_name(&self, name: &str) -> bool {
        let name_bytes = name.as_bytes();
        if name_bytes.len() != self.len as usize {
            return false;
        }
        &self.name[0..name_bytes.len()] == name_bytes
    }

    pub fn set_name(&mut self, name: &str) {
        let name_bytes = name.as_bytes();
        self.len = name_bytes.len() as u8;
        self.name[..name_bytes.len()].copy_from_slice(name_bytes);
    }
}

#[allow(clippy::from_over_into)]
impl Into<VfsDirEntry> for &DirEntry {
    fn into(self) -> VfsDirEntry {
        let name = &self.name[0..self.len as usize];
        let name_str = str::from_utf8(name).unwrap_or("non-utf8 name");
        VfsDirEntry {
            inode: self.inode as VfsInodeIndex,
            name: name_str.to_string().into_boxed_str(),
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
struct InodeInfo {
    size: u64,
    levels: u8,
    type_flags: InodeTypeAndPerms,
    owner_uid: u16,
    owner_gid: u16,
    link_count: u16,
    creation_seconds_since_epoch: u64,
    modification_seconds_since_epoch: u64,
    stat_change_seconds_since_epoch: u64,
}

const _: () = {
    assert!(core::mem::size_of::<InodeInfo>() <= 512);
};

impl InodeInfo {
    #[allow(clippy::wrong_self_convention)]
    fn into_vfs(&self, index: InodeIndex, rfs: &Rfs2) -> VfsInode {
        VfsInode {
            index: index as VfsInodeIndex,
            device: rfs.partition.partition.device,
            type_mode: self.type_flags.clone(),
            link_cnt: self.link_count,
            uid: self.owner_uid,
            gid: self.owner_gid,
            size: self.size,
            access_time: 0,
            modification_time: self.modification_seconds_since_epoch,
            stat_change_time: self.stat_change_seconds_since_epoch,
        }
    }

    fn from_vfs(existing_info: InodeInfo, inode: &VfsInode) -> Self {
        //only allow changing some fields like owner uid/gid, times and flags
        Self {
            size: existing_info.size,
            levels: existing_info.levels,
            type_flags: inode.type_mode.clone(),
            owner_uid: inode.uid,
            owner_gid: inode.gid,
            link_count: existing_info.link_count,
            creation_seconds_since_epoch: existing_info.creation_seconds_since_epoch,
            modification_seconds_since_epoch: inode.modification_time,
            stat_change_seconds_since_epoch: inode.stat_change_time,
        }
    }
}

const _: () = {
    assert!(core::mem::size_of::<InodeInfo>() <= 512);
};

impl Rfs2 {
    async fn get_file_root_block(&self, inode_index: InodeIndex) -> Result<BlockPtr, ErrorCode> {
        let lock = self.inode_lock.lock().await;
        let Some(block_index) = BTreeNode::find_inode_root(inode_index, self).await else {
            println!("inode {} not found in tree", inode_index);
            return Err(ErrorCode::InodeNotPresent);
        };
        drop(lock);
        Ok(block_index)
    }

    async fn get_file_info(&self, file_root: BlockPtr) -> InodeInfo {
        let working_buffer = WorkingBlock::new();
        self.partition
            .read(file_root as usize * BLOCK_SIZE_SECTORS, 1, &[working_buffer.phys.0])
            .await;
        let inode_info = working_buffer.get_as::<InodeInfo>().clone();
        working_buffer.forget_mem_binding();
        inode_info
    }

    async fn set_file_info(&self, file_root: BlockPtr, info: InodeInfo) {
        let mut working_buffer = WorkingBlock::new();
        let inode_info = working_buffer.get_as_mut::<InodeInfo>();
        *inode_info = info;
        self.partition
            .write(file_root as usize * BLOCK_SIZE_SECTORS, 1, &[working_buffer.phys.0])
            .await;
        working_buffer.forget_mem_binding();
    }
}

#[async_trait::async_trait]
impl FileSystem for Rfs2 {
    fn device_id(&self) -> DeviceId {
        self.partition.partition.device
    }
    fn partition_id(&self) -> uuid::Uuid {
        self.partition.partition.part_id
    }

    async fn unmount(&self) -> Result<(), ErrorCode> {
        //nothing to do rn
        Ok(())
    }
    ///Offset must be page aligned
    async fn read(
        &self,
        inode: VfsInodeIndex,
        offset_bytes: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
    ) -> Result<u64, ErrorCode> {
        if !offset_bytes.is_multiple_of(4096) {
            panic!("non-page-aligned offset not yet supported");
        }
        for phys in buffer {
            if phys.0 % 4096 != 0 {
                return Err(ErrorCode::InvalidArgument);
            }
        }

        let lock = self.get_file_lock(inode as InodeIndex);
        let locked = lock.lock();

        let file_root = self.get_file_root_block(inode as InodeIndex).await?;
        let offset_blocks = offset_bytes / 4096;
        let res = self.read_locked(file_root, offset_blocks, size_bytes, buffer).await;
        drop(locked);
        res
    }

    async fn read_dir(&self, inode: VfsInodeIndex) -> Result<Box<[VfsDirEntry]>, ErrorCode> {
        let lock = self.get_file_lock(inode as InodeIndex);
        let locked = lock.lock();

        let file_root = self.get_file_root_block(inode as InodeIndex).await?;
        let file_info = self.get_file_info(file_root).await;
        let size_pages = file_info.size.div_ceil(4096);

        let buf = physical_allocator::allocate_contiguous(size_pages as u32);
        let buf_vec = buf.get_range().get_addresses().collect::<Vec<_>>();
        let res = self
            .read_locked(file_root, 0, file_info.size, &buf_vec)
            .await
            .expect("correct arguments ig");

        drop(locked);

        let buf_virt = VirtAddr::from(buf.0.start);
        let num_dir_entries = res as usize / core::mem::size_of::<DirEntry>();
        let direntry_slice = unsafe { core::slice::from_raw_parts(buf_virt.0 as *const DirEntry, num_dir_entries) };

        let vfs_direntries: Vec<VfsDirEntry> = direntry_slice.iter().map(|dent| dent.into()).collect();

        Ok(vfs_direntries.into_boxed_slice())
    }

    ///Offset must be page aligned. Returns the new inode
    async fn write(
        &self,
        inode: VfsInodeIndex,
        offset_bytes: u64,
        size: u64,
        buffer: &[PhysAddr],
    ) -> Result<(VfsInode, u64), ErrorCode> {
        if !offset_bytes.is_multiple_of(4096) {
            panic!("non-page-aligned offset not yet supported");
        }
        for phys in buffer {
            if phys.0 % 4096 != 0 {
                return Err(ErrorCode::InvalidArgument);
            }
        }

        let lock = self.get_file_lock(inode as InodeIndex);
        let locked = lock.lock();

        let file_root = self.get_file_root_block(inode as InodeIndex).await?;

        let offset_blocks = offset_bytes / 4096;
        let written = self.write_locked(file_root, offset_blocks, size, buffer).await?;
        drop(locked);

        let file_info = self.get_file_info(file_root).await;
        Ok((file_info.into_vfs(inode as InodeIndex, self), written))
    }

    async fn stat(&self, inode: VfsInodeIndex, _parent: VfsInodeIndex) -> Result<VfsInode, ErrorCode> {
        let file_root = self.get_file_root_block(inode as InodeIndex).await?;
        let inode_info = self.get_file_info(file_root).await;
        Ok(inode_info.into_vfs(inode as InodeIndex, self))
    }

    async fn set_stat(&self, inode_index: VfsInodeIndex, _parent: VfsInodeIndex, inode_data: VfsInode) -> Result<(), ErrorCode> {
        let lock = self.get_file_lock(inode_index as InodeIndex);
        let locked = lock.lock();

        let file_root = self.get_file_root_block(inode_index as InodeIndex).await?;
        let existing_info = self.get_file_info(file_root).await;
        let mut new_info = InodeInfo::from_vfs(existing_info, &inode_data);

        let since_epoch = std::time::Instant::now().duration_since(std::time::UNIX_EPOCH).as_secs();
        new_info.stat_change_seconds_since_epoch = since_epoch;

        self.set_file_info(file_root, new_info).await;
        drop(locked);
        Ok(())
    }

    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(
        &self,
        name: &str,
        parent_dir: VfsInodeIndex,
        type_mode: InodeTypeAndPerms,
        uid: u16,
        gid: u16,
    ) -> Result<(VfsInode, VfsInode), ErrorCode> {
        let new_block = self.allocate_block().await;
        let new_inode = self.allocate_inode().await;

        let inode_lock = self.inode_lock.lock().await;
        BTreeNode::insert_inode_root(new_inode, new_block, self).await;
        drop(inode_lock);

        let since_epoch = std::time::Instant::now().duration_since(std::time::UNIX_EPOCH).as_secs();
        let new_inode_info = InodeInfo {
            size: 0,
            levels: 0,
            type_flags: type_mode,
            owner_uid: uid,
            owner_gid: gid,
            link_count: 0,
            creation_seconds_since_epoch: since_epoch,
            modification_seconds_since_epoch: since_epoch,
            stat_change_seconds_since_epoch: since_epoch,
        };

        self.set_file_info(new_block, new_inode_info.clone()).await;

        println!("created new file, linking");
        let res = self.link(new_inode as VfsInodeIndex, parent_dir, name).await;
        match res {
            Err(e) => {
                self.release_block(new_block).await;
                self.release_inode(new_inode).await;
                return Err(e);
            }
            Ok((parent_inode, child_inode)) => {
                println!("linking done successfully");
                Ok((parent_inode, child_inode))
            }
        }
    }

    async fn unlink(&self, parent_inode: VfsInodeIndex, name: &str) -> Result<(VfsInode, VfsInode), ErrorCode> {
        if name.len() > 256 {
            return Err(ErrorCode::InvalidArgument);
        }
        if name.is_empty() {
            return Err(ErrorCode::InvalidArgument);
        }

        let parent_lock = self.get_file_lock(parent_inode as InodeIndex);
        let parent_locked = parent_lock.lock();

        let (binding, entries) = self.read_direntries(parent_inode).await;
        let Some(pos) = entries.iter_mut().position(|ent| ent.is_name(name)) else {
            drop(parent_locked);
            return Err(ErrorCode::InodeNotPresent);
        };

        let child_inode = entries[pos].inode;

        let child_lock = self.get_file_lock(child_inode);
        let child_locked = child_lock.lock();

        entries[pos] = entries[entries.len() - 2].clone();
        entries[entries.len() - 2] = entries[entries.len() - 1].clone();

        self.write_direntries(parent_inode, binding.0, entries.len() - 2).await;

        let parent_root = self.get_file_root_block(parent_inode as InodeIndex).await?;
        let parent_info = self.get_file_info(parent_root).await;
        let parent_vfs_inode = parent_info.into_vfs(parent_inode as InodeIndex, self);

        let child_root = self.get_file_root_block(child_inode).await?;
        let mut child_info = self.get_file_info(child_root).await;
        child_info.link_count -= 1;

        let child_vfs_inode = child_info.into_vfs(child_inode, self);

        drop(child_locked);
        drop(parent_locked);

        Ok((parent_vfs_inode, child_vfs_inode))
    }

    async fn remove_inode(&self, inode: VfsInodeIndex) -> Result<(), ErrorCode> {
        let lock = self.get_file_lock(inode as InodeIndex);
        let locked = lock.lock();

        let inode_root = self.get_file_root_block(inode as InodeIndex).await?;
        let inode_info = self.get_file_info(inode_root).await;

        if inode_info.link_count > 0 {
            return Err(ErrorCode::InvalidArgument);
        }

        self.truncate_locked(inode_root, 0).await;

        let inode_lock = self.inode_lock.lock().await;
        BTreeNode::remove_key_root(inode as InodeIndex, self).await;
        self.release_inode(inode as InodeIndex).await;
        drop(inode_lock);

        self.release_block(inode_root).await;
        drop(locked);

        Ok(())
    }

    ///returns the new parent inode
    async fn link(
        &self,
        child_inode: VfsInodeIndex,
        parent_inode: VfsInodeIndex,
        name: &str,
    ) -> Result<(VfsInode, VfsInode), ErrorCode> {
        if name.len() > 256 {
            return Err(ErrorCode::InvalidArgument);
        }
        if name.is_empty() {
            return Err(ErrorCode::InvalidArgument);
        }

        let lock = self.get_file_lock(parent_inode as InodeIndex);
        let locked = lock.lock();
        let child_lock = self.get_file_lock(child_inode as InodeIndex);
        let child_locked = child_lock.lock();

        //check if child inode exists
        let inode_lock = self.inode_lock.lock().await;
        let child_root_res = BTreeNode::find_inode_root(child_inode as InodeIndex, self).await;
        drop(inode_lock);
        if child_root_res.is_none() {
            return Err(ErrorCode::InodeNotPresent);
        }

        let parent_root = self.get_file_root_block(parent_inode as InodeIndex).await?;
        let child_root = self.get_file_root_block(child_inode as InodeIndex).await?;

        let (binding, entries) = self.read_direntries(parent_root).await;

        for entry in entries.iter() {
            if entry.inode == 0 {
                continue;
            }

            if entry.is_name(name) {
                drop(locked);
                return Err(ErrorCode::InvalidArgument);
            }
        }

        let last = entries.last_mut().expect("exists");

        last.inode = child_inode as InodeIndex;
        last.set_name(name);

        self.write_direntries(parent_root, binding.0, entries.len()).await;

        let mut child_info = self.get_file_info(child_root).await;
        child_info.link_count += 1;
        let new_child_inode = child_info.into_vfs(child_inode as InodeIndex, self);
        self.set_file_info(child_root, child_info).await;

        let parent_info = self.get_file_info(parent_root).await;
        let new_parent_inode = parent_info.into_vfs(parent_inode as InodeIndex, self);

        drop(locked);
        drop(child_locked);

        Ok((new_parent_inode, new_child_inode))
    }

    async fn truncate(&self, inode: VfsInodeIndex, size: u64) -> Result<(), ErrorCode> {
        let lock = self.get_file_lock(inode as InodeIndex);
        let locked = lock.lock();

        let file_root = self.get_file_root_block(inode as InodeIndex).await?;
        self.truncate_locked(file_root, size as usize).await;

        drop(locked);

        Ok(())
    }

    async fn rename(&self, inode: VfsInodeIndex, parent_inode: VfsInodeIndex, name: &str) -> Result<(), ErrorCode> {
        if name.len() > 256 {
            return Err(ErrorCode::InvalidArgument);
        }
        if name.is_empty() {
            return Err(ErrorCode::InvalidArgument);
        }

        let lock = self.get_file_lock(parent_inode as InodeIndex);
        let locked = lock.lock();

        let (binding, entries) = self.read_direntries(parent_inode).await;
        let Some(entry) = entries.iter_mut().find(|ent| ent.inode == inode as InodeIndex) else {
            drop(locked);
            return Err(ErrorCode::InodeNotPresent);
        };
        entry.set_name(name);

        self.write_direntries(parent_inode, binding.0, entries.len() - 1).await;

        Ok(())
    }
}
