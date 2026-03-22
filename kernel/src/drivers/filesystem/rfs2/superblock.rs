use crate::drivers::filesystem::rfs2::{BlockPtr, InodeIndex};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(in crate::drivers::filesystem::rfs2) struct SuperBlock {
    pub inode_mask_ptr: BlockPtr,
    pub inode_tree_root_ptr: BlockPtr,
    pub fs_size: u64,
    pub root_inode_index: InodeIndex,
    pub checksum: u32,
}

impl SuperBlock {
    pub fn check_checksum(&self) -> bool {
        let mut checksum = 0u32;
        for i in (0..core::mem::size_of::<SuperBlock>()).step_by(4) {
            let part = self as *const SuperBlock as *const u32;
            let part = unsafe { part.byte_add(i).read() };
            checksum = checksum ^ part;
        }
        checksum == 0
    }

    pub fn calculate_checksum(&mut self) {
        self.checksum = 0;
        let mut checksum = 0u32;
        for i in (0..core::mem::size_of::<SuperBlock>()).step_by(4) {
            let part = self as *const SuperBlock as *const u32;
            let part = unsafe { part.byte_add(i).read() };
            checksum = checksum ^ part;
        }
        self.checksum = checksum;
    }
}
