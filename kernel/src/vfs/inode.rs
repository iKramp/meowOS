use bitfield::bitfield;

use super::{DeviceId, InodeIndex};

//this is returned by the stat() syscall
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Inode {
    pub index: InodeIndex,
    pub device: DeviceId, //some map to major/minor (minor are partitions)
    pub type_mode: InodeType,
    pub link_cnt: u16,
    pub uid: u16,
    pub gid: u16,
    ///len of a symlink is the length of the pathname
    pub size: u64,
    pub access_time: u64,
    pub modification_time: u64,
    pub stat_change_time: u64,
    //available if this represents a device, otherwise inherits from device
    pub preferred_block_size: u16,
    ///number of blocks used by this inode, in 512 byte units!!!!!
    pub blocks: u32,
}

const FILE_MODE_MASK: u32 = 0xFFF00000;
const FILE_TYPE_MASK: u32 = 0xF000;
const PERM_MASK: u32 = 0x1FF;
const TEST: u32 = 0o4000;
//use this: https://man7.org/linux/man-pages/man7/inode.7.html
//internal fs inode types may differ (as there is no need for socket, block device,...) but rfs
//uses this
#[derive(Debug, Clone)]
pub struct InodeType(u32);

impl InodeType {
    pub fn get_flags(&self) -> InodeFlags {
        InodeFlags(self.0 & PERM_MASK)
    }

    pub fn is_socket(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o140000
    }

    pub fn is_symlink(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o120000
    }

    pub fn is_file(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o100000
    }

    pub fn is_dir(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o40000
    }

    pub fn is_block_device(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o60000
    }

    pub fn is_char_device(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o20000
    }

    pub fn is_fifo(&self) -> bool {
        self.0 & FILE_TYPE_MASK == 0o10000
    }

    pub fn new_dir(perms: u32) -> Self {
        InodeType(0o40000 | perms)
    }

    pub fn new_file(perms: u32) -> Self {
        InodeType(perms)
    }
}

//unused for now, we don't need permissions
bitfield! {
    pub struct InodeFlags(u32);
    impl Debug;
    pub suid, set_suid: 11;
    pub sgid, set_sgid: 10;
    pub sticky, set_sticky: 9;

    pub r_usr, set_r_usr: 8;
    pub w_usr, set_w_usr: 7;
    pub x_usr, set_x_usr: 6;

    pub r_grp, set_r_grp: 5;
    pub w_grp, set_w_grp: 4;
    pub x_grp, set_x_grp: 3;

    pub r_othr, set_r_othr: 2;
    pub w_othr, set_w_othr: 1;
    pub x_othr, set_x_othr: 0;
}
