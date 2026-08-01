use crate::memory::addresses::*;
use std::{boxed::Box, error::ErrorCode, vec::Vec};

use crate::{
    drivers::filesystem::rfs2::{BLOCK_SIZE_SECTORS, BlockPtr, Rfs2, operations::PTRS_PER_BLOCK},
    memory::physical_allocator,
};

impl Rfs2 {
    #[heap_future::heap_future]
    pub(super) async fn write_locked(
        &self,
        file_root: BlockPtr,
        offset_blocks: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
    ) -> Result<u64, ErrorCode> {
        if size_bytes == 0 {
            return Ok(0);
        }
        if size_bytes.div_ceil(4096) > buffer.len() as u64 {
            return Err(ErrorCode::InvalidArgument);
        }

        self.increase_file_size(file_root, offset_blocks as usize * 4096 + size_bytes as usize)
            .await;

        let file_info = self.get_file_info(file_root).await;

        let working_block = physical_allocator::allocate_contiguous(1);
        self.partition
            .read(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                7,
                &working_block.0.get_addresses().collect::<Vec<_>>(),
            )
            .await;

        let small_file = file_info.levels == 0;
        if small_file {
            let to_write = file_info.size.min(size_bytes);

            self.partition
                .write(file_root as usize * BLOCK_SIZE_SECTORS + 1, BLOCK_SIZE_SECTORS - 1, buffer)
                .await;
            return Ok(to_write as u64);
        }

        //they must be contiguous both physically and virtually
        let mut current_range = working_block;

        let first_block_to_write = offset_blocks;
        let last_block_to_write = (offset_blocks + size_bytes / 4096).min(file_info.size / 4096);

        let mut current_ptr_level = 1;
        let levels = file_info.levels;
        loop {
            let level_diff = levels - current_ptr_level;
            let first_relevant_ptr = first_block_to_write / (PTRS_PER_BLOCK.pow(level_diff as u32) as u64);
            let last_relevant_ptr = last_block_to_write / (PTRS_PER_BLOCK.pow(level_diff as u32) as u64);

            let ptr_virt = VirtAddr::from(current_range.0.start) + first_relevant_ptr * core::mem::size_of::<BlockPtr>() as u64;
            let ptrs_to_read = (last_relevant_ptr - first_relevant_ptr + 1) as usize;
            let ptrs_slice = unsafe { core::slice::from_raw_parts(ptr_virt.0 as *const BlockPtr, ptrs_to_read) };

            if current_ptr_level == levels {
                for (ptr, phys) in ptrs_slice.iter().zip(buffer.iter()) {
                    self.partition
                        .write(*ptr as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[*phys])
                        .await;
                }
                break;
            } else {
                let new_working_range = physical_allocator::allocate_contiguous(ptrs_to_read as u32);
                current_range = new_working_range;

                self.read_locked_pointers(ptrs_slice, &current_range.0.get_addresses().collect::<Vec<_>>())
                    .await;

                current_ptr_level += 1;
            }
        }

        Ok((last_block_to_write - first_block_to_write) * 4096)
    }
}
