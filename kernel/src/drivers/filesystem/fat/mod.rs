use bitfield::bitfield;
use core::{mem::MaybeUninit, slice};
use std::{kerror, println, string::String, sync::arc::Arc, vec::Vec};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::MountedPartition,
    memory::{addresses::*, physical_allocator},
    vfs::{DeviceId, FileSystem, FileSystemFactory, Inode, InodeIndex, InodeTypeAndPerms},
};

use std::{boxed::Box, error::KernelError};

use crate::drivers::block_device::disk::DirEntry;

#[allow(non_snake_case)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
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

bitfield! {
    struct DirAttrs(u8);
    impl Debug;
    read_only, _: 0;
    hidden, _: 1;
    system, _: 2;
    volume_id, _: 3;
    directory, _: 4;
    archive, _: 5;
}

impl DirAttrs {
    fn has_long_file_name(&self) -> bool {
        self.0 & 0x0F == 0x0F
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
struct FatDirEntry {
    dir_name: [u8; 11],
    dir_attr: DirAttrs,
    useless: u8,
    crt_time_subsecond: u8,
    crt_time: u16,
    crt_date: u16,
    useless_2: u16,
    first_cluster_high: u16,
    useless_3: u16,
    useless_4: u16,
    first_cluster_low: u16,
    file_size: u32,
}

impl FatDirEntry {
    fn is_used(&self) -> bool {
        self.dir_name[0] != 0x00 && self.dir_name[0] != 0xE5
    }

    fn has_entries_later(&self) -> bool {
        self.dir_name[0] != 0x00
    }

    fn has_long_name(&self) -> bool {
        self.dir_attr.read_only() && self.dir_attr.hidden() && self.dir_attr.system() && self.dir_attr.volume_id()
    }
}

pub(super) fn init_fat() {
    crate::vfs::register_filesystem_driver_factory(Arc::new(FatFactory));
}

pub struct FatFactory;

impl FatFactory {
    pub const UUID: Uuid = Uuid::from_u128(0xebd0a0a2b9e5443387c068b6b72699c7);
}

#[async_trait::async_trait]
impl FileSystemFactory for FatFactory {
    async fn mount(&self, partition: MountedPartition) -> Arc<dyn FileSystem + Send> {
        Arc::new(FatDriver::new(partition).await)
    }

    fn uuid(&self) -> Uuid {
        Self::UUID
    }

    fn name(&self) -> &str {
        "FAT32"
    }
}

#[derive(Debug)]
struct FatDriver {
    //write once
    partition: MountedPartition,
    header: FatHeader,
    fat_start_sector: u32,
    fat_sectors: u32,
    root_dir_start_sector: u32,
    data_start_sector: u32,
    data_sectors: u32,
}

unsafe impl Sync for FatDriver {}

impl FatDriver {
    async fn new(partition: MountedPartition) -> Self {
        let page = physical_allocator::allocate();
        partition.read(0, 1, &[page.0]).await;

        let mut header = MaybeUninit::uninit();

        unsafe {
            let header_ptr = header.as_mut_ptr();
            let page_virt = VirtAddr::from(&page);
            let src_ptr = page_virt.0 as *const FatHeader;
            *header_ptr = *src_ptr;

            let header_ref = header.assume_init_ref();

            if header_ref.BPB_BytesPerSector != 512 {
                panic!("FAT32 driver only supports 512 bytes per sector");
            }

            if header_ref.BPB_BytesPerSector * header_ref.BPB_SectorsPerCluster as u16 > 32 * 1024 {
                panic!("FAT32 driver only supports up to 32KB clusters");
            }

            if header_ref.BPB_RootEntryCount != 0 {
                panic!("FAT32 driver only supports FAT32, not FAT12 or FAT16");
            }
            if header_ref.BPB_TotalSectors16 != 0 {
                panic!("FAT32 driver only supports FAT32, not FAT12 or FAT16");
            }
            if header_ref.BPB_FATSize16 != 0 {
                panic!("FAT32 driver only supports FAT32, not FAT12 or FAT16");
            }

            let fat_size = header_ref.BPB_FATSize32;
            let total_sectors = header_ref.BPB_TotalSectors32;

            let fat_start_sector = header_ref.BPB_ReservedSectorCount as u32;

            let fat_sectors = fat_size * header_ref.BPB_NumFATs as u32;

            let root_cluster = header_ref.BPB_RootCluster;
            let root_dir_sectors = (32_u32 * header_ref.BPB_RootEntryCount as u32).div_ceil(header_ref.BPB_BytesPerSector as u32);
            if root_dir_sectors != 0 {
                panic!("FAT32 driver only supports FAT32, not FAT12 or FAT16");
            }

            let data_start_sector = header_ref.BPB_ReservedSectorCount as u32 + (header_ref.BPB_NumFATs as u32 * fat_size);
            let data_sectors = total_sectors - data_start_sector;

            let root_dir_start_sector = data_start_sector + ((root_cluster - 2) * header_ref.BPB_SectorsPerCluster as u32);

            let count_of_clusters = data_sectors / header_ref.BPB_SectorsPerCluster as u32;

            if count_of_clusters < 4085 {
                panic!("FAT12 is not supported");
            } else if count_of_clusters < 65525 {
                panic!("FAT16 is not supported");
            }

            let part = Self {
                partition,
                header: header.assume_init(),
                fat_start_sector,
                fat_sectors,
                root_dir_start_sector,
                data_start_sector,
                data_sectors,
            };

            println!("fat_start_sector: {}", fat_start_sector);
            println!("fat_sectors: {}", fat_sectors);
            println!("root_dir_start_sector: {}", root_dir_start_sector);
            println!("root_dir_sectors: {}", root_dir_sectors);
            println!("data_start_sector: {}", data_start_sector);
            println!("data_sectors: {}", data_sectors);
            println!("header: {:#?}", header_ref);

            drop(page);

            part
        }
    }

    fn get_sector_from_cluster(&self, cluster: u32) -> u32 {
        println!("Translating cluster {} to sector", cluster);
        let res = self.data_start_sector + ((cluster - 2) * self.header.BPB_SectorsPerCluster as u32);
        println!("Cluster {} translates to sector {}", cluster, res);
        res
    }

    fn get_entry_sec_offset(&self, entry: u32) -> (u32, u32) {
        let sec_num = self.header.BPB_ReservedSectorCount as u32 + (entry * 4 / self.header.BPB_BytesPerSector as u32);
        let offset = (entry * 4) % self.header.BPB_BytesPerSector as u32;
        (sec_num, offset)
    }

    async fn read_sector(&self, sector: u32) -> Box<[u8; 512]> {
        let page = physical_allocator::allocate();
        let page_virt = VirtAddr::from(&page);
        self.partition.read(sector as usize, 1, &[page.0]).await;
        let mut data = Box::new([0_u8; 512]);
        let data_src = page_virt.0 as *const u8;
        let data_dest = data.as_mut_ptr();
        unsafe {
            data_dest.copy_from(data_src, 512);
        }
        drop(page);
        data
    }

    async fn read_fat_entry(&self, entry: u32) -> u32 {
        let (sector, offset) = self.get_entry_sec_offset(entry);
        let data = self.read_sector(sector).await;
        let data_ptr = data.as_ptr() as *const u32;
        let entry_val = unsafe { data_ptr.byte_add(offset as usize).read() };
        entry_val & 0x0FFFFFFF
    }

    fn entry_is_final(entry_val: u32) -> bool {
        entry_val >= 0x0FFFFFF8
    }

    async fn read_file_sector(&self, sector: u32, file_cluster_start: u32) -> Option<Box<[u8; 512]>> {
        let nth_entry = sector / self.header.BPB_SectorsPerCluster as u32;
        let mut curr_cluster = file_cluster_start;
        for _ in 0..nth_entry {
            if curr_cluster == 0 {
                return None;
            }
            if Self::entry_is_final(curr_cluster) {
                return None;
            }
            curr_cluster = self.read_fat_entry(curr_cluster).await;
        }
        if curr_cluster == 0 || Self::entry_is_final(curr_cluster) {
            return None;
        }

        let sector_in_cluster = sector % self.header.BPB_SectorsPerCluster as u32;
        let cluster_start_sector = self.get_sector_from_cluster(curr_cluster);
        Some(self.read_sector(cluster_start_sector + sector_in_cluster).await)
    }

    async fn read_dir_internal(&self, inode_index: InodeIndex) -> Result<Box<[FatDirEntry]>, KernelError> {
        let mut sector_offset = 0;
        let mut buf = Vec::new();
        loop {
            let Some(data) = self.read_file_sector(sector_offset, inode_index as u32).await else {
                return Ok(buf.into_boxed_slice());
            };
            sector_offset += 1;

            let src_ptr = data.as_ptr() as *const FatDirEntry;
            let len = 512 / core::mem::size_of::<FatDirEntry>();
            let entry_slice = unsafe { slice::from_raw_parts(src_ptr, len) };
            for entry in entry_slice.iter() {
                if entry.has_long_name() {
                    continue;
                }
                if entry.is_used() {
                    buf.push(entry.clone());
                }
                if !entry.has_entries_later() {
                    return Ok(buf.into_boxed_slice());
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl FileSystem for FatDriver {
    fn device_id(&self) -> DeviceId {
        self.partition.partition.device
    }
    fn partition_id(&self) -> Uuid {
        self.partition.partition.part_id
    }

    async fn unmount(&self) -> Result<(), KernelError> {
        Ok(())
    }
    ///Offset must be page aligned
    async fn read(&self, inode: InodeIndex, offset_bytes: u64, size_bytes: u64, buffer: &[PhysAddr]) -> Result<u64, KernelError> {
        if !offset_bytes.is_multiple_of(512) {
            return kerror!(IllegalValue);
        }

        let size_sectors = size_bytes.div_ceil(512);
        let size_sectors = size_sectors.min(buffer.len() as u64 * 8);

        println!(
            "reading {} bytes ({} sectors) from inode {} at offset {}",
            size_bytes, size_sectors, inode, offset_bytes
        );

        let start_sector = offset_bytes / 512;

        for i in start_sector..(start_sector + size_sectors) {
            let buffer_sector = i - start_sector;

            let Some(data) = self.read_file_sector(i as u32, inode as u32).await else {
                return Ok((buffer_sector * 512).min(size_bytes));
            };
            let buffer_phys = buffer[buffer_sector as usize / 8];
            let buffer_virt: VirtAddr = buffer_phys.into();
            let in_buffer_offset = (buffer_sector % 8) * 512;
            let ptr_dest = (buffer_virt + in_buffer_offset).0 as *mut u8;
            let ptr_src = data.as_ptr();
            unsafe {
                ptr_dest.copy_from(ptr_src, 512);
            }
        }

        return Ok((size_sectors * 512).min(size_bytes));
    }
    async fn read_dir(&self, inode: InodeIndex) -> Result<Box<[DirEntry]>, KernelError> {
        let entries = self.read_dir_internal(inode).await?;
        let vfs_entries: Vec<DirEntry> = entries
            .iter()
            .map(|entry| {
                let entry_cluster = entry.first_cluster_low as u32 | ((entry.first_cluster_high as u32) << 16);

                let base = unsafe { str::from_utf8_unchecked(&entry.dir_name[..8]).trim() };
                let extension = unsafe { str::from_utf8_unchecked(&entry.dir_name[8..]).trim() };
                let mut final_string = String::new();
                final_string.push_str(base);
                if !extension.is_empty() {
                    final_string.push('.');
                    final_string.push_str(extension);
                }
                final_string.make_ascii_lowercase();
                println!("returning entry {}", &final_string);
                DirEntry {
                    inode: entry_cluster as u64,
                    name: final_string.into_boxed_str(),
                }
            })
            .filter(|entry| *entry.name != *"." && *entry.name != *"..")
            .collect();

        return Ok(vfs_entries.into_boxed_slice());
    }
    ///Offset must be page aligned. Returns the new inode
    async fn write(
        &self,
        _inode: InodeIndex,
        _offset: u64,
        _size: u64,
        _buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    async fn stat(&self, inode: InodeIndex, parent: InodeIndex) -> Result<Inode, KernelError> {
        if inode == self.header.BPB_RootCluster as u64 {
            return Ok(Inode {
                index: inode,
                device: self.device_id(),
                type_mode: InodeTypeAndPerms::new_dir(0o555),
                link_cnt: 1,
                uid: 0,
                gid: 0,
                size: 0,
                access_time: 0,
                modification_time: 0,
                stat_change_time: 0,
            });
        }

        let parent_entries = self.read_dir_internal(parent).await?;
        for entry in &parent_entries {
            println!("in stat, potential entry: {:?}", entry)
        }
        println!("trying to find inode {}", inode);
        let Some(entry_to_find) = parent_entries
            .iter()
            .find(|e| (e.first_cluster_low as u32 | ((e.first_cluster_high as u32) << 16)) == inode as u32)
        else {
            return kerror!(NoEntry);
        };

        let is_dir = entry_to_find.dir_attr.directory();
        let type_perms = if is_dir {
            InodeTypeAndPerms::new_dir(0o555) //r-wr-wr-w
        } else {
            InodeTypeAndPerms::new_file(0o555) //r-wr-wr-w
        };

        Ok(Inode {
            index: inode,
            device: self.device_id(),
            type_mode: type_perms,
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: entry_to_find.file_size as u64,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
        })
    }
    async fn set_stat(&self, _inode_index: InodeIndex, _parent: InodeIndex, _inode_data: Inode) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(
        &self,
        _name: &str,
        _parent_dir: InodeIndex,
        _type_mode: InodeTypeAndPerms,
        _uid: u16,
        _gid: u16,
    ) -> Result<(Inode, Inode), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    //returns the new inodes (parent, child). Reaching link count 0 doesn't remove the file yet
    async fn unlink(&self, _parent_inode: InodeIndex, _name: &str) -> Result<(Inode, Inode), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    //removes the inode and all its data. Link count has to be 0
    async fn remove_inode(&self, _inode: InodeIndex) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    ///returns the new inodes (parent, child)
    async fn link(&self, _inode: InodeIndex, _parent_dir: InodeIndex, _name: &str) -> Result<(Inode, Inode), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    async fn truncate(&self, _inode: InodeIndex, _size: u64) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    async fn rename(&self, _inode: InodeIndex, _parent_inode: InodeIndex, _name: &str) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
}
