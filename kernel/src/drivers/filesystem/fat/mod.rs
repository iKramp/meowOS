use std::{mem_utils::PhysAddr, sync::arc::Arc};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::MountedPartition,
    vfs::{DeviceId, FileSystem, FileSystemFactory, Inode, InodeIndex, InodeTypeAndPerms},
};

use std::{boxed::Box, error::ErrorCode};

use crate::drivers::block_device::disk::DirEntry;

#[allow(non_snake_case)]
#[repr(C, packed)]
struct FatHeader {
    BS_JmpBoot: [u8; 3],
    BS_OEMName: [u8; 8],
    BPB_BytesPerSector: u16,
    BPB_SectorsPerCluster: u8,
    BPB_ReservedSectorCount: u16,
    BPB_NumFATs: u8,
    BPB_RootEntryCount: u16,
    BPB_TotalSectors16: u16, //unused for fat32
    BPB_Media: u8,
    BPB_FATSize16: u16, //unused for fat32
    BPB_SectorsPerTrack: u16,
    BPB_NumHeads: u16,
    BPB_HiddenSectors: u32,
    BPB_TotalSectors32: u32,
    BPB_FATSize32: u32,
    BPB_ExtFlags: u16,
    BPB_FSVer: u16,
    BPB_RootCluster: u32,
    BPB_FSInfo: u16,
    BPB_BkBootSec: u16,
    BPB_Reserved: [u8; 12],
    BS_DrvNum: u8,
    BS_Reserved1: u8,
    BS_BootSig: u8,
    BS_VolID: u32,
    BS_VolLab: [u8; 11],
    BS_FilSysType: [u8; 8],
}

struct FullFatHeader {
    header: FatHeader,
    boot_code: [u8; 420],
    signature: [u8; 2],
}

pub(super) fn init_fat() {
    crate::vfs::register_filesystem_driver_factory(Arc::new(FatFactory));
}

pub struct FatFactory;

impl FatFactory {
    pub const UUID: Uuid = Uuid::from_u128(0x2477786763f94f0391447b0cad53daad);
}

#[async_trait::async_trait]
impl FileSystemFactory for FatFactory {
    async fn mount(&self, partition: MountedPartition) -> Arc<dyn FileSystem + Send> {
        Arc::new(FatDriver::new(partition).await)
    }

    fn uuid(&self) -> Uuid {
        Self::UUID
    }
}

#[derive(Debug)]
struct FatDriver {
    //write once
    partition: MountedPartition,
}

unsafe impl Sync for FatDriver {}

impl FatDriver {
    async fn new(partition: MountedPartition) -> Self {
        Self { partition }
    }
}

#[async_trait::async_trait]
impl FileSystem for FatDriver {
    fn device_id(&self) -> DeviceId {
        self.partition.partition.device
    }
    async fn unmount(&self) -> Result<(), ErrorCode> {
        Ok(())
    }
    ///Offset must be page aligned
    async fn read(&self, inode: InodeIndex, offset_bytes: u64, size_bytes: u64, buffer: &[PhysAddr]) -> Result<u64, ErrorCode> {
        todo!()
    }
    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode> {
        todo!()
    }
    ///Offset must be page aligned. Returns the new inode
    async fn write(&self, inode: InodeIndex, offset: u64, size: u64, buffer: &[PhysAddr]) -> Result<(Inode, u64), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    async fn stat(&self, inode: InodeIndex) -> Result<Inode, ErrorCode> {
        todo!()
    }
    async fn set_stat(&self, _inode_index: InodeIndex, _inode_data: Inode) -> Result<(), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(
        &self,
        _name: &str,
        _parent_dir: InodeIndex,
        _type_mode: InodeTypeAndPerms,
        _uid: u16,
        _gid: u16,
    ) -> Result<(Inode, Inode), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    //returns the new inodes (parent, child). Reaching link count 0 doesn't remove the file yet
    async fn unlink(&self, _parent_inode: InodeIndex, _name: &str) -> Result<(Inode, Inode), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    //removes the inode and all its data. Link count has to be 0
    async fn remove_inode(&self, _inode: InodeIndex) -> Result<(), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    ///returns the new inodes (parent, child)
    async fn link(&self, _inode: InodeIndex, _parent_dir: InodeIndex, _name: &str) -> Result<(Inode, Inode), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    async fn truncate(&self, _inode: InodeIndex, _size: u64) -> Result<(), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
    async fn rename(&self, _inode: InodeIndex, _parent_inode: InodeIndex, _name: &str) -> Result<(), ErrorCode> {
        return Err(ErrorCode::UnsupportedOperation);
    }
}
