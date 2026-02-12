use crate::vfs::{self, InodeType};
use bitfield::bitfield;

mod btree;
#[allow(clippy::module_inception)]
mod rfs;
pub use rfs::*;

use crate::drivers::block_device::disk::{Partition, MountedPartition};

const BLOCK_SIZE_SECTORS: usize = 8;
const VIRTUAL_ONLY: bool = true;

#[repr(C)]
#[derive(Debug)]
struct SuperBlock {
    ///Block index of the inode tree
    pub inode_tree: u32,
    ///Block index of the inode bitmask
    pub inode_bitmask: u32,
}

#[repr(C)]
#[derive(Debug, Clone)]
struct DirEntry {
    pub inode: u32,
    pub name: [u8; 128],
}

//1 inode per block, contains the file if it's small enough, otherwise pointers to blocks, pointers
//  to pointers, etc. File or pointers start at next sector
#[repr(C)]
#[derive(Debug)]
struct Inode {
    size: InodeSize,
    inode_type_mode: InodeType,
    link_count: u16,
    uid: u16,
    gid: u16,
    access_time: u64,
    modification_time: u64,
    stat_change_time: u64,
}

const _: () = {
    assert!(core::mem::size_of::<Inode>() <= 512);
};

impl Inode {
    fn to_vfs(&self, index: u32, partition: &Partition) -> vfs::Inode {
        vfs::Inode {
            index: index as u64,
            device: partition.device,
            type_mode: self.inode_type_mode.clone(),
            link_cnt: self.link_count,
            uid: self.uid,
            gid: self.gid,
            size: self.size.size(),
            preferred_block_size: 4096,
            blocks: self.size.size().div_ceil(4096) as u32,
            access_time: self.access_time,
            modification_time: self.modification_time,
            stat_change_time: self.stat_change_time,
        }
    }

    ///only for changing permissions and similar. Does not update size, link count and other things
    ///100% dependent on the filesystem
    fn from_vfs(vfs_inode: vfs::Inode, link_count: u16, size: InodeSize) -> Self {
        Self {
            inode_type_mode: vfs_inode.type_mode,
            uid: vfs_inode.uid,
            gid: vfs_inode.gid,
            access_time: vfs_inode.access_time,
            modification_time: vfs_inode.access_time,
            stat_change_time: vfs_inode.stat_change_time,
            link_count,
            size,
        }
    }
}

bitfield! {
    struct InodeSize(u64);
    impl Debug;
    ///Size in bytes. Block length is size / 4096 rounded up
    pub size, set_size: 50, 0;
    ///number of levels of pointers. 0 means the file is small enough to fit in
    ///the inode block, 1 means pointers to blocks, 2 means pointers to pointers to blocks, etc
    pub ptr_levels, set_ptr_levels: 63, 62;
}

//max size: block size
struct GroupHeader {
    bitmask: [u8; 4096],
}

impl GroupHeader {
    pub fn new() -> Self {
        Self { bitmask: [0; 4096] }
    }

    pub fn find_empty(&self) -> Option<usize> {
        for (i, byte) in self.bitmask.iter().enumerate() {
            if *byte != u8::MAX {
                for j in 0..8 {
                    if (byte & (1 << j)) == 0 {
                        return Some(i * 8 + j);
                    }
                }
            }
        }
        None
    }

    pub fn set(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        self.bitmask[byte] |= 1 << bit;
    }

    pub fn clear(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        self.bitmask[byte] &= !(1 << bit);
    }
}

//can fit 32736 (0x7FE0) inodes
#[repr(C)]
struct InodeBitmask {
    inodes: [u8; 4092],
    next_ptr: u32,
}

impl InodeBitmask {
    pub fn new() -> Self {
        Self {
            inodes: [0; 4092],
            next_ptr: 0,
        }
    }

    pub fn find_empty(&self) -> Option<usize> {
        for (i, byte) in self.inodes.iter().enumerate() {
            if *byte != u8::MAX {
                for j in 0..8 {
                    if (byte & (1 << j)) == 0 {
                        return Some(i * 8 + j);
                    }
                }
            }
        }
        None
    }

    pub fn set(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        self.inodes[byte] |= 1 << bit;
    }

    pub fn clear(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        self.inodes[byte] &= !(1 << bit);
    }
}
