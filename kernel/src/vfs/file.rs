use std::sync::async_lock::AsyncSpinlock;

use bitfield::bitfield;

use super::{InodeIdentifier, InodeIdentifierChain};

pub type FileDescriptor = u64;

#[derive(Debug)]
pub struct FileHandle {
    pub inode: InodeIdentifier,
    pub parent_chain: InodeIdentifierChain,
    pub position: u64,
    pub file_flags: FileFlags,
}

bitfield! {
    #[derive(PartialEq, Eq)]
    pub struct FileFlags(u8);
    impl Debug;
    pub read, set_read: 0;
    pub write, set_write: 1;
    pub append, set_append: 2;
    //bit 3 is create, not a file bit
    //bit 4 is truncate, not a file bit
    pub dir, set_dir: 5;
}

impl FileFlags {
    pub const fn new() -> Self {
        FileFlags(0)
    }

    pub fn new_with_flags(read: bool, write: bool, append: bool, dir: bool) -> Self {
        let mut flags = FileFlags::new();
        if read {
            flags.set_read(true);
        }
        if write {
            flags.set_write(true);
        }
        if append {
            flags.set_append(true);
        }
        if dir {
            flags.set_dir(true);
        }
        flags
    }

    pub fn with_read(mut self, read: bool) -> Self {
        self.set_read(read);
        self
    }

    pub fn with_write(mut self, write: bool) -> Self {
        self.set_write(write);
        self
    }

    pub fn with_append(mut self, append: bool) -> Self {
        self.set_append(append);
        self
    }
}
