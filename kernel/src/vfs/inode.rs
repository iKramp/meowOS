use bitfield::bitfield;

use super::{DeviceId, InodeIndex};

//this is returned by the stat() syscall
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Inode {
    //RO
    pub index: InodeIndex,
    //RO
    pub device: DeviceId, //some map to major/minor (minor are partitions)
    //type RO, permissions RW
    pub type_mode: InodeTypeAndPerms,
    //RW
    pub link_cnt: u16,
    //RW
    pub uid: u16,
    //RW
    pub gid: u16,
    //RW
    pub size: u64,
    //RW
    pub access_time: u64,
    //RW
    pub modification_time: u64,
    //RW
    pub stat_change_time: u64,
}

impl Inode {
    pub fn update_from(&mut self, other: &Inode) {
        self.type_mode = other.type_mode.clone();
        self.link_cnt = other.link_cnt;
        self.uid = other.uid;
        self.gid = other.gid;
        self.size = other.size;
        self.access_time = other.access_time;
        self.modification_time = other.modification_time;
        self.stat_change_time = other.stat_change_time;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InodeType {
    File = 0,        //--\
    Directory = 1,   //------real file types
    Symlink = 2,     //--/
    Socket = 3,      //--\
    BlockDevice = 4, //---\
    CharDevice = 5,  //------mental illnesses
    Fifo = 6,        //---/
}

impl InodeType {
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(InodeType::File),
            1 => Some(InodeType::Directory),
            2 => Some(InodeType::Symlink),
            3 => Some(InodeType::Socket),
            4 => Some(InodeType::BlockDevice),
            5 => Some(InodeType::CharDevice),
            6 => Some(InodeType::Fifo),
            _ => None,
        }
    }

    pub fn to_id(&self) -> u32 {
        match self {
            InodeType::File => 0,
            InodeType::Directory => 1,
            InodeType::Symlink => 2,
            InodeType::Socket => 3,
            InodeType::BlockDevice => 4,
            InodeType::CharDevice => 5,
            InodeType::Fifo => 6,
        }
    }
}

const PERM_MASK: u32 = 0xFF_FF_FF;
//use this: https://man7.org/linux/man-pages/man7/inode.7.html
///The top 8 bits represent the file type [`InodeType`] (bit shifted)
///The bottom 24 bits represent [`InodePermissionFlags`]
#[derive(Debug, Clone)]
pub struct InodeTypeAndPerms(u32);

impl InodeTypeAndPerms {
    pub fn get_perms(&self) -> InodePermissionFlags {
        InodePermissionFlags(self.0 & PERM_MASK)
    }

    pub fn inode_type(&self) -> Option<InodeType> {
        InodeType::from_id(self.0 >> 24)
    }

    pub fn new(inode_type: InodeType, perms: InodePermissionFlags) -> Self {
        InodeTypeAndPerms((inode_type.to_id() << 24) | perms.0)
    }

    pub fn is_socket(&self) -> bool {
        self.inode_type() == Some(InodeType::Socket)
    }

    pub fn is_symlink(&self) -> bool {
        self.inode_type() == Some(InodeType::Symlink)
    }

    pub fn is_file(&self) -> bool {
        self.inode_type() == Some(InodeType::File)
    }

    pub fn is_dir(&self) -> bool {
        self.inode_type() == Some(InodeType::Directory)
    }

    pub fn is_block_device(&self) -> bool {
        self.inode_type() == Some(InodeType::BlockDevice)
    }

    pub fn is_char_device(&self) -> bool {
        self.inode_type() == Some(InodeType::CharDevice)
    }

    pub fn is_fifo(&self) -> bool {
        self.inode_type() == Some(InodeType::Fifo)
    }

    pub fn new_dir(perms: u32) -> Self {
        InodeTypeAndPerms((InodeType::Directory.to_id() << 24) | perms)
    }

    pub fn new_file(perms: u32) -> Self {
        InodeTypeAndPerms(perms)
    }
}

//unused for now, we don't need permissions
//NOTE: only use the bottom 24 bits, the top 8 are for the file type
bitfield! {
    pub struct InodePermissionFlags(u32);
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
