use std::{
    mem_utils::{self, PhysAddr},
    vec::Vec,
};

use crate::{
    drivers::filesystem::rfs2::{BlockPtr, InodeIndex, Rfs2, operations::DirEntry},
    memory::physical_allocator,
};

impl Rfs2 {
    //contains an additional allocation for linking
    pub(super) async fn read_direntries(&self, dir_root: BlockPtr) -> (PhysAddr, &'static mut [DirEntry]) {
        let dir_info = self.get_file_info(dir_root).await;
        let size_pages = (dir_info.size + core::mem::size_of::<DirEntry>() as u64).div_ceil(4096);

        let buf = physical_allocator::allocate_contiguius_high(size_pages);
        let buf_virt = mem_utils::translate_phys_virt_addr(buf);
        let buf_vec = (0..size_pages).map(|i| buf + i * 4096).collect::<Vec<_>>();
        self.read_locked(dir_root, 0, dir_info.size, &buf_vec)
            .await
            .expect("correct args");
        let entries = dir_info.size as usize / core::mem::size_of::<DirEntry>();
        let entry_slice = unsafe { core::slice::from_raw_parts_mut(buf_virt.0 as *mut DirEntry, entries + 1) };
        let last = entry_slice.last_mut().expect("must have at least 1 entry");
        last.inode = 0;
        last.name = [0; 256 - core::mem::size_of::<InodeIndex>() - 1];
        (buf, entry_slice)
    }

    pub(super) async fn write_direntries(&self, dir_root: BlockPtr, buffer: PhysAddr, num_entries: usize) {
        let num_bytes = num_entries * core::mem::size_of::<DirEntry>();
        let num_pages = num_bytes.div_ceil(4096) as u64;
        let buffer_vec = (0..num_pages).map(|i| buffer + i * 4096).collect::<Vec<_>>();
        self.write_locked(
            dir_root,
            0,
            num_entries as u64 * core::mem::size_of::<DirEntry>() as u64,
            &buffer_vec,
        )
        .await
        .expect("correct args");
        self.truncate_locked(dir_root, num_entries * core::mem::size_of::<DirEntry>())
            .await;

        Self::dealloc_dirent_binding(buffer, num_entries);
    }

    pub(super) fn dealloc_dirent_binding(binding: PhysAddr, num_entries: usize) {
        let num_bytes = num_entries * core::mem::size_of::<DirEntry>();
        let num_pages = num_bytes.div_ceil(4096) as u64;
        for i in 0..num_pages {
            unsafe { physical_allocator::deallocate_frame(binding + i * 4096) };
        }
    }
}
