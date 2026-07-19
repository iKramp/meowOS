use crate::memory::addresses::*;
use std::{error::ErrorCode, println, vec::Vec};

use crate::{
    drivers::filesystem::rfs2::{BLOCK_SIZE_SECTORS, BlockPtr, Rfs2, operations::PTRS_PER_BLOCK},
    memory::physical_allocator,
};

impl Rfs2 {
    pub(super) async fn read_locked_pointers(&self, pointers: &[BlockPtr], buffer: &[PhysAddr]) {
        let mut curr_ptr = pointers[0];
        let mut curr_index = 0;
        let mut curr_size = 1;
        for ptr in pointers.iter().skip(1) {
            if *ptr == curr_ptr + 1 {
                println!(
                    "pointer {} is contiguous with current pointer {}, increasing size to read",
                    ptr, curr_ptr
                );
                curr_size += 1;
                continue;
            }
            self.partition
                .read(
                    curr_ptr as usize * BLOCK_SIZE_SECTORS,
                    curr_size * BLOCK_SIZE_SECTORS,
                    &buffer[curr_index..(curr_index + curr_size)],
                )
                .await;
            curr_ptr = *ptr;
            curr_index += curr_size;
            curr_size = 1;
        }
        self.partition
            .read(
                curr_ptr as usize * BLOCK_SIZE_SECTORS,
                curr_size * BLOCK_SIZE_SECTORS,
                &buffer[curr_index..(curr_index + curr_size)],
            )
            .await;
    }

    pub(super) async fn read_locked(
        &self,
        file_root: BlockPtr,
        offset_blocks: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
    ) -> Result<u64, ErrorCode> {
        if size_bytes == 0 {
            println!("size is 0, nothing to read");
            return Ok(0);
        }
        if size_bytes.div_ceil(4096) > buffer.len() as u64 {
            println!(
                "buffer too small for requested size: buf_len {} blocks, size {} bytes",
                buffer.len(),
                size_bytes
            );
            return Err(ErrorCode::InvalidArgument);
        }

        let file_info = self.get_file_info(file_root).await;

        if offset_blocks * 4096 > file_info.size {
            println!("offset is beyond file size, nothing to read");
            return Ok(0);
        }

        let working_block = physical_allocator::allocate_contiguous(1);
        self.partition
            .read(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                BLOCK_SIZE_SECTORS - 1,
                &working_block.0.get_addresses().collect::<Vec<_>>(),
            )
            .await;

        let small_file = file_info.levels == 0;
        if small_file {
            println!(
                "small file read from block {} with offset {} blocks and size {}B",
                file_root, offset_blocks, size_bytes
            );

            let src_virt = VirtRange::from(&working_block);
            let dest_virt = VirtAddr::from(buffer[0]);

            unsafe { core::ptr::copy_nonoverlapping(src_virt.start.0 as *const u8, dest_virt.0 as *mut u8, size_bytes as usize) };

            return Ok(file_info.size.min(size_bytes) as u64);
        }
        println!("reading multi-level file");

        //they must be contiguous both physically and virtually
        let mut current_range = working_block;

        //inclusive bounds of blocks to read
        let first_block_to_read = offset_blocks;
        let last_block_to_read = (offset_blocks + (size_bytes - 1) / 4096).min(file_info.size.saturating_sub(1) / 4096);

        let mut current_ptr_level = 1;
        let levels = file_info.levels;
        let mut skipped_blocks = 0;
        loop {
            let level_diff = levels - current_ptr_level;
            let ptr_blocks = PTRS_PER_BLOCK.pow(level_diff as u32) as u64;
            let first_relevant_ptr = (first_block_to_read - skipped_blocks) / ptr_blocks;
            let last_relevant_ptr = (last_block_to_read - skipped_blocks) / ptr_blocks;
            skipped_blocks += ptr_blocks * first_relevant_ptr;

            let ptr_virt = VirtAddr::from(current_range.0.start) + first_relevant_ptr * core::mem::size_of::<BlockPtr>() as u64;
            let ptrs_to_read = (last_relevant_ptr - first_relevant_ptr + 1) as usize;
            let ptrs_slice = unsafe { core::slice::from_raw_parts(ptr_virt.0 as *const BlockPtr, ptrs_to_read) };

            if current_ptr_level == levels {
                self.read_locked_pointers(ptrs_slice, buffer).await;

                break;
            } else {
                let new_working_physical = physical_allocator::allocate_contiguous(ptrs_to_read as u32);
                current_range = new_working_physical;
                self.read_locked_pointers(ptrs_slice, &current_range.0.get_addresses().collect::<Vec<_>>())
                    .await;

                current_ptr_level += 1;
            }
        }

        Ok((last_block_to_read - first_block_to_read + 1) * 4096)
    }
}
