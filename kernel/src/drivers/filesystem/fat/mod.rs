#![allow(clippy::slow_vector_initialization)]

use bitfield::bitfield;
use core::{
    mem::MaybeUninit,
    slice,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};
use std::{alloc::string::ToString, format, kerror, kerror_unwrapped, println, string::String, sync::arc::Arc, vec::Vec};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::MountedPartition,
    memory::{addresses::*, physical_allocator},
    vfs::{
        DeviceId, FileReadResult, FileSystem, FileSystemFactory, Inode, InodeExtraData, InodeIndex, InodeTypeAndPerms,
        file::FileHandleExtraData,
    },
};

use std::{boxed::Box, error::KernelError};

use crate::drivers::block_device::disk::DirEntry;

const USE_TREE_CACHE: bool = true;

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

#[repr(C)]
#[derive(Debug, Clone)]
struct FatLongDirEntry {
    order: u8,
    name1: [u8; 10],
    attr: DirAttrs,
    type_: u8,
    checksum: u8,
    name2: [u8; 12],
    first_cluster_low: u16,
    name3: [u8; 4],
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

    fn to_long_entry(&self) -> &FatLongDirEntry {
        debug_assert!(self.has_long_name(), "to_long_entry called on non-long entry");
        unsafe { &*(self as *const FatDirEntry as *const FatLongDirEntry) }
    }
}

impl FatLongDirEntry {
    fn get_name(&self) -> String {
        let name1 = char::decode_utf16(
            self.name1
                .chunks(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .filter(|&c| c != 0xFFFF && c != 0x0000),
        )
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect::<String>();
        let name2 = char::decode_utf16(
            self.name2
                .chunks(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .filter(|&c| c != 0xFFFF && c != 0x0000),
        )
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect::<String>();
        let name3 = char::decode_utf16(
            self.name3
                .chunks(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .filter(|&c| c != 0xFFFF && c != 0x0000),
        )
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect::<String>();
        format!("{}{}{}", name1, name2, name3)
    }
}

struct PageCache {
    page_data: OwnedPhysAddr,
    start_sector: u64,
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
            if header_ref.BPB_ExtFlags & (1 << 7) != 0 {
                panic!("don't support non-mirrored FAT");
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

    fn get_sector_from_cluster(&self, cluster: u32) -> u64 {
        self.data_start_sector as u64 + ((cluster as u64 - 2) * self.header.BPB_SectorsPerCluster as u64)
    }

    fn get_fat_entry_sec_offset(&self, entry: u32) -> (u64, u32) {
        let sec_num = self.header.BPB_ReservedSectorCount as u64 + (entry as u64 * 4 / self.header.BPB_BytesPerSector as u64);
        let offset = (entry * 4) % self.header.BPB_BytesPerSector as u32;
        (sec_num, offset)
    }

    async fn read_sector(&self, sector: u64, cache: &mut PageCache, data: &mut [u8; 512]) {
        let page_virt = VirtAddr::from(cache.page_data.0);

        if sector < cache.start_sector || sector - cache.start_sector >= 8 {
            let to_read = sector & !0b111;
            self.partition.read(to_read as usize, 8, &[cache.page_data.0]).await;
            cache.start_sector = to_read;
        }

        let src_off = (sector - cache.start_sector) * 512;

        let data_src = (page_virt.0 + src_off) as *const u8;
        let data_dest = data.as_mut_ptr();
        unsafe {
            data_dest.copy_from(data_src, 512);
        }
    }

    async fn read_fat_entry(&self, entry_index: u32, cache: &mut PageCache, data: &mut [u8; 512]) -> u32 {
        let (sector, offset) = self.get_fat_entry_sec_offset(entry_index);
        self.read_sector(sector, cache, data).await;
        //my allocator should handle this fine
        debug_assert!(data.as_ptr() as usize % 4 == 0, "data pointer is not aligned to 4 bytes");
        let data_ptr = data.as_ptr() as *const u32;
        unsafe { data_ptr.byte_add(offset as usize).read() }
    }

    fn entry_is_final(entry_val: u32) -> bool {
        entry_val >= 0x0FFFFFF8
    }

    ///chain_follow_count: How many times to follow the chain. 0 means noop
    ///chain_start_disk_cluster: The starting cluster in the disk to follow the chain from
    ///chain_start_file_cluster: which cluster of the disk in order is the start of this chain
    async fn get_disk_cluster_from_file_cluster(
        &self,
        chain_follow_count: u32,
        chain_start_disk_cluster: u32,
        chain_start_file_cluster: u32,
        cache: &mut PageCache,
        data: &mut [u8; 512],
        inode_extra_data: Option<&FatInodeExtraData>,
    ) -> Option<u32> {
        let mut curr_cluster = chain_start_disk_cluster;
        for i in 0..chain_follow_count {
            curr_cluster = self.read_fat_entry(curr_cluster, cache, data).await;
            curr_cluster &= 0x0FFFFFFF; //mask to 28 bits, as per FAT32 spec
            if curr_cluster == 0 || Self::entry_is_final(curr_cluster) {
                return None;
            }

            if !USE_TREE_CACHE {
                continue;
            }

            let Some(inode_extra_data) = inode_extra_data else {
                continue;
            };

            let cache_chunk = (chain_start_file_cluster + i + 1) / inode_extra_data.tree_chunk_clusters;
            let mut data = ExtraDataTreeEntry(0);
            data.set_valid(true);
            data.set_file_cluster((chain_start_file_cluster + i + 1) as u64);
            data.set_disk_cluster(curr_cluster as u64);
            inode_extra_data.cluster_tree[cache_chunk as usize].store(data.0, Relaxed);
        }
        Some(curr_cluster)
    }

    async fn read_file_sector(
        &self,
        sector: u32,
        file_cluster_start: u32,
        cache: &mut PageCache,
        data: &mut [u8; 512],
    ) -> Option<u32> {
        //cluster
        let nth_cluster = sector / self.header.BPB_SectorsPerCluster as u32;

        let curr_cluster = self
            .get_disk_cluster_from_file_cluster(nth_cluster, file_cluster_start, 0, cache, data, None)
            .await?;

        let sector_in_cluster = sector as u64 % self.header.BPB_SectorsPerCluster as u64;
        let cluster_start_sector = self.get_sector_from_cluster(curr_cluster);
        self.read_sector(cluster_start_sector + sector_in_cluster, cache, data).await;
        Some(curr_cluster)
    }

    async fn read_dir_internal(&self, inode_index: InodeIndex) -> Result<Box<[(FatDirEntry, Option<String>)]>, KernelError> {
        let mut sector_offset = 0;
        let mut buf = Vec::new();
        let mut curr_long_name: Option<String> = None;
        let mut data = [0_u8; 512];
        let mut page_cache = PageCache {
            page_data: physical_allocator::allocate(),
            start_sector: u64::MAX,
        };
        loop {
            //don't wanna deal with optimization
            let Some(_cluster) = self
                .read_file_sector(sector_offset, inode_index as u32, &mut page_cache, &mut data)
                .await
            else {
                return Ok(buf.into_boxed_slice());
            };
            sector_offset += 1;

            let src_ptr = data.as_ptr() as *const FatDirEntry;
            let len = 512 / core::mem::size_of::<FatDirEntry>();
            let entry_slice = unsafe { slice::from_raw_parts(src_ptr, len) };
            for entry in entry_slice.iter() {
                if entry.has_long_name() {
                    let long_entry = entry.to_long_entry();
                    let entry_name = long_entry.get_name();

                    match &mut curr_long_name {
                        Some(name) => {
                            name.insert_str(0, &entry_name);
                        }
                        None => {
                            curr_long_name = Some(entry_name.to_string());
                        }
                    }
                }
                if !entry.has_long_name() {
                    if entry.is_used() {
                        buf.push((entry.clone(), curr_long_name.take()));
                    }
                    curr_long_name = None; //reset just in case
                    if !entry.has_entries_later() {
                        return Ok(buf.into_boxed_slice());
                    }
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

    fn f_handle_open(
        &self,
        inode: InodeIndex,
        _inode_data: &dyn InodeExtraData,
    ) -> Result<Box<dyn FileHandleExtraData>, KernelError> {
        let file_handle_data = FatFileHandleExtraData {
            cluster_start: inode as u32,
        };

        Ok(Box::new(file_handle_data))
    }

    fn f_handle_close(&self, _inode: InodeIndex, _extra_data: Box<dyn FileHandleExtraData>) -> Result<(), KernelError> {
        //nothing to free
        Ok(())
    }

    async fn unmount(&self) -> Result<(), KernelError> {
        Ok(())
    }
    ///Offset must be page aligned
    async fn read(
        &self,
        _inode: InodeIndex,
        f_handle_extra_data: &dyn FileHandleExtraData,
        _inode_extra_data: &dyn InodeExtraData,
        offset_bytes: u64,
        size_to_read_bytes: u64,
        buffer: &[PhysAddr],
        _blocking: bool,
    ) -> Result<(u64, FileReadResult), KernelError> {
        if !offset_bytes.is_multiple_of(512) {
            return kerror!(IllegalValue);
        }
        if size_to_read_bytes == 0 {
            return Ok((0, FileReadResult::Normal));
        }
        if size_to_read_bytes.div_ceil(4096) != buffer.len() as u64 {
            return kerror!(IllegalValue);
        }

        let mut data = [0_u8; 512];
        let mut page_cache = PageCache {
            page_data: physical_allocator::allocate(),
            start_sector: u64::MAX,
        };

        let size_to_read_sectors = size_to_read_bytes.div_ceil(512);
        let handle_extra_data = f_handle_extra_data
            .as_any()
            .downcast_ref::<FatFileHandleExtraData>()
            .ok_or(kerror_unwrapped!(IllegalValue))?;
        let inode_extra_data = _inode_extra_data
            .as_any()
            .downcast_ref::<FatInodeExtraData>()
            .ok_or(kerror_unwrapped!(IllegalValue))?;

        let offset_sectors = offset_bytes / self.header.BPB_BytesPerSector as u64;

        let mut starting_file_cluster = offset_sectors / self.header.BPB_SectorsPerCluster as u64;

        let cluster_tree_chunk = starting_file_cluster as u32 / inode_extra_data.tree_chunk_clusters;
        let mut starting_disk_cluster = None;
        if USE_TREE_CACHE {
            for i in (0..cluster_tree_chunk).rev() {
                let Some(data) = inode_extra_data.cluster_tree.get(i as usize) else {
                    continue;
                };
                let data = ExtraDataTreeEntry(data.load(Relaxed));
                if data.valid() {
                    starting_file_cluster = data.file_cluster();
                    starting_disk_cluster = Some(data.disk_cluster() as u32);
                    break;
                }
            }
        }

        let mut previous_file_cluster = starting_file_cluster as u32;
        let mut previous_disk_cluster = if let Some(a) = starting_disk_cluster {
            a
        } else {
            self.get_disk_cluster_from_file_cluster(
                previous_file_cluster,
                handle_extra_data.cluster_start,
                0,
                &mut page_cache,
                &mut data,
                Some(inode_extra_data),
            )
            .await
            .ok_or(kerror_unwrapped!(InternalFSError))?
        };

        let mut total_sectors_read = 0;

        for i in 0..size_to_read_sectors {
            let curr_sector = offset_sectors + i;
            let curr_file_cluster = (curr_sector / self.header.BPB_SectorsPerCluster as u64) as u32;
            let curr_disk_cluster = if curr_file_cluster == previous_file_cluster {
                previous_disk_cluster
            } else {
                //next cluster. Use old as a reference
                let Some(cluster) = self
                    .get_disk_cluster_from_file_cluster(
                        curr_file_cluster - previous_file_cluster,
                        previous_disk_cluster,
                        previous_file_cluster,
                        &mut page_cache,
                        &mut data,
                        Some(inode_extra_data),
                    )
                    .await
                else {
                    total_sectors_read = i;
                    break;
                };
                cluster
            };

            let sector_in_cluster = curr_sector % self.header.BPB_SectorsPerCluster as u64;

            let curr_disk_sector = self.get_sector_from_cluster(curr_disk_cluster) + sector_in_cluster;

            self.read_sector(curr_disk_sector, &mut page_cache, &mut data).await;

            let buffer_phys = buffer[i as usize / 8];
            let buffer_virt = VirtAddr::from(buffer_phys);
            let in_buffer_offset = (i % 8) * 512;
            let ptr_dest = (buffer_virt + in_buffer_offset).0 as *mut u8;

            let len_to_copy = if i == (size_to_read_sectors - 1) {
                ((size_to_read_bytes - 1) % 512) + 1 //modulo but 1-512, not 0-511
            } else {
                512
            };

            let dest_slice = unsafe { core::slice::from_raw_parts_mut(ptr_dest, len_to_copy as usize) };

            dest_slice.copy_from_slice(&data[..(len_to_copy as usize)]);

            previous_file_cluster = curr_file_cluster;
            previous_disk_cluster = curr_disk_cluster;
            total_sectors_read = i + 1;
        }

        let result = if total_sectors_read == size_to_read_sectors {
            FileReadResult::Normal
        } else {
            #[cfg(debug_assertions)]
            panic!();
            #[allow(unreachable_code)]
            FileReadResult::Invalid
        };

        return Ok(((total_sectors_read * 512).min(size_to_read_bytes), result));
    }

    async fn read_dir(&self, inode: InodeIndex, _inode_extra_data: &dyn InodeExtraData) -> Result<Box<[DirEntry]>, KernelError> {
        let entries = self.read_dir_internal(inode).await?;
        let vfs_entries: Vec<DirEntry> = entries
            .into_iter()
            .map(|(entry, long_name)| {
                let entry_cluster = entry.first_cluster_low as u32 | ((entry.first_cluster_high as u32) << 16);

                let base = unsafe { str::from_utf8_unchecked(&entry.dir_name[..8]).trim() };
                let extension = unsafe { str::from_utf8_unchecked(&entry.dir_name[8..]).trim() };
                let final_string = match long_name {
                    Some(name) => name,
                    None => {
                        let mut final_string = String::new();
                        final_string.push_str(base);
                        if !extension.is_empty() {
                            final_string.push('.');
                            final_string.push_str(extension);
                        }
                        final_string.make_ascii_lowercase();
                        final_string
                    }
                };
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
        _f_handle_extra_data: &dyn FileHandleExtraData,
        _inode_extra_data: &dyn InodeExtraData,
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
                internal_synchronization: false,
                requires_aligned_position: true,
                extra_data: Box::new(FatInodeExtraData {
                    parent_inode: parent,
                    size_bytes: 0,
                    size_clusters: 0,
                    cluster_tree: Box::new([]),
                    tree_chunk_clusters: 0,
                }),
            });
        }

        let parent_entries = self.read_dir_internal(parent).await?;
        for entry in &parent_entries {
            println!("in stat, potential entry: {:?}", entry)
        }
        println!("trying to find inode {}", inode);
        let Some(entry_to_find) = parent_entries
            .iter()
            .find(|(e, _long_name)| (e.first_cluster_low as u32 | ((e.first_cluster_high as u32) << 16)) == inode as u32)
            .map(|(e, _long_name)| e)
        else {
            return kerror!(NoEntry);
        };

        let is_dir = entry_to_find.dir_attr.directory();
        let type_perms = if is_dir {
            InodeTypeAndPerms::new_dir(0o555) //r-wr-wr-w
        } else {
            InodeTypeAndPerms::new_file(0o555) //r-wr-wr-w
        };

        let size_sectors = entry_to_find.file_size.div_ceil(self.header.BPB_BytesPerSector as u32);
        let size_clusters = size_sectors.div_ceil(self.header.BPB_SectorsPerCluster as u32);
        let cluster_tree_size = get_cluster_tree_size(size_clusters);
        let tree_chunk_clusters = size_clusters.div_ceil(cluster_tree_size as u32);
        let mut cluster_tree_vec = Vec::with_capacity(cluster_tree_size);
        cluster_tree_vec.resize_with(cluster_tree_size, || AtomicU64::new(0));
        let cluster_tree = cluster_tree_vec.into_boxed_slice();

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
            internal_synchronization: false,
            requires_aligned_position: true,
            extra_data: Box::new(FatInodeExtraData {
                parent_inode: parent,
                size_bytes: entry_to_find.file_size,
                size_clusters,
                cluster_tree,
                tree_chunk_clusters,
            }),
        })
    }
    async fn set_stat(
        &self,
        _inode_index: InodeIndex,
        _inode_data: Inode,
        _extra_data: &dyn InodeExtraData,
    ) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    ///returns the new parent inode in the first field and the new inode in the second
    async fn create(
        &self,
        _name: &str,
        _parent_dir: InodeIndex,
        _parent_inode_extra_data: &dyn InodeExtraData,
        _type_mode: InodeTypeAndPerms,
        _uid: u16,
        _gid: u16,
    ) -> Result<(Inode, Inode), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    //returns the new inodes (parent, child). Reaching link count 0 doesn't remove the file yet
    async fn unlink(
        &self,
        _parent_inode: InodeIndex,
        _parent_inode_extra_data: &dyn InodeExtraData,
        _name: &str,
    ) -> Result<(Inode, Inode), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    //removes the inode and all its data. Link count has to be 0
    async fn remove_inode(&self, _inode: InodeIndex) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    ///returns the new inodes (parent, child)
    async fn link(
        &self,
        _inode: InodeIndex,
        _parent_dir: InodeIndex,
        _parent_inode_extra_data: &dyn InodeExtraData,
        _name: &str,
    ) -> Result<(Inode, Inode), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    async fn truncate(
        &self,
        _inode: InodeIndex,
        _f_handle_extra_data: &dyn FileHandleExtraData,
        _inode_extra_data: &dyn InodeExtraData,
        _size: u64,
    ) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
    async fn rename(
        &self,
        _inode: InodeIndex,
        _parent_inode: InodeIndex,
        _parent_f_handle_extra_data: &dyn FileHandleExtraData,
        _parent_inode_extra_data: &dyn InodeExtraData,
        _name: &str,
    ) -> Result<(), KernelError> {
        return kerror!(UnsupportedOperation);
    }
}

bitfield! {
    struct ExtraDataTreeEntry(u64);
    impl Debug;
    valid, set_valid: 0;
    file_cluster, set_file_cluster: 31, 1;
    disk_cluster, set_disk_cluster: 63, 32;
}

#[derive(Debug)]
struct FatFileHandleExtraData {
    cluster_start: u32,
}

#[derive(Debug)]
struct FatInodeExtraData {
    parent_inode: InodeIndex,
    size_bytes: u32,
    size_clusters: u32,
    tree_chunk_clusters: u32,
    cluster_tree: Box<[AtomicU64]>,
}

impl crate::vfs::InodeExtraData for FatInodeExtraData {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}
impl crate::vfs::file::FileHandleExtraData for FatFileHandleExtraData {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

fn get_cluster_tree_size(size_clusters: u32) -> usize {
    let res = size_clusters.isqrt();
    let res = if res * res < size_clusters { res + 1 } else { res };
    res as usize
}
