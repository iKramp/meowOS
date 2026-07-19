use crate::memory::addresses::*;
use std::vec::Vec;

use crate::{
    drivers::filesystem::rfs2::{BlockPtr, InodeIndex, Rfs2, operations::DirEntry},
    memory::physical_allocator,
};

impl Rfs2 {
    //contains an additional allocation for linking
    pub(super) async fn read_direntries(&self, dir_root: BlockPtr) -> (OwnedPhysRange, &'static mut [DirEntry]) {
        let dir_info = self.get_file_info(dir_root).await;
        let size_pages = (dir_info.size + core::mem::size_of::<DirEntry>() as u64).div_ceil(4096);

        let buf = physical_allocator::allocate_contiguous(size_pages as u32);
        let buf_virt = VirtRange::from(&buf);
        self.read_locked(dir_root, 0, dir_info.size, &buf.0.get_addresses().collect::<Vec<_>>())
            .await
            .expect("correct args");
        let entries = dir_info.size as usize / core::mem::size_of::<DirEntry>();
        let entry_slice = unsafe { core::slice::from_raw_parts_mut(buf_virt.start.0 as *mut DirEntry, entries + 1) };
        let last = entry_slice.last_mut().expect("must have at least 1 entry");
        last.inode = 0;
        last.name = [0; 256 - core::mem::size_of::<InodeIndex>() - 1];
        (buf, entry_slice)
    }

    pub(super) async fn write_direntries(&self, dir_root: BlockPtr, buffer: PhysRange, num_entries: usize) {
        self.write_locked(
            dir_root,
            0,
            num_entries as u64 * core::mem::size_of::<DirEntry>() as u64,
            &buffer.get_addresses().collect::<Vec<_>>(),
        )
        .await
        .expect("correct args");
        self.truncate_locked(dir_root, num_entries * core::mem::size_of::<DirEntry>())
            .await;
    }
}
