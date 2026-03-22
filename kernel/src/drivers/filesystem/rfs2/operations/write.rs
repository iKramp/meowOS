use std::{
    error::ErrorCode,
    mem_utils::{self, PhysAddr},
    vec::Vec,
};

use crate::{
    drivers::filesystem::rfs2::{BLOCK_SIZE_SECTORS, BlockPtr, Rfs2, operations::PTRS_PER_BLOCK},
    memory::physical_allocator,
};

impl Rfs2 {
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

        let file_info = self.get_file_info(file_root).await;

        self.increase_file_size(file_root, offset_blocks as usize * 4096 + size_bytes as usize)
            .await;

        let working_block = physical_allocator::allocate_frame();
        self.partition
            .read(file_root as usize * BLOCK_SIZE_SECTORS + 1, 7, &[working_block])
            .await;

        let small_file = file_info.levels == 0;
        if small_file {
            let to_write = file_info.size.min(size_bytes);

            self.partition.write(file_root as usize * BLOCK_SIZE_SECTORS + 1, BLOCK_SIZE_SECTORS - 1, buffer).await;
            return Ok(to_write as u64);
        }

        //they must be contiguous both physically and virtually
        let mut current_working_blocks = Vec::new();
        current_working_blocks.push(working_block);

        let first_block_to_write = offset_blocks;
        let last_block_to_write = (offset_blocks + size_bytes / 4096).min(file_info.size / 4096);

        let mut current_ptr_level = 1;
        let levels = file_info.levels;
        loop {
            let level_diff = levels - current_ptr_level;
            let first_relevant_ptr = first_block_to_write / (PTRS_PER_BLOCK.pow(level_diff as u32) as u64);
            let last_relevant_ptr = last_block_to_write / (PTRS_PER_BLOCK.pow(level_diff as u32) as u64);

            let ptr_virt =
                mem_utils::translate_phys_virt_addr(*current_working_blocks.first().expect("must have at least 1 block"))
                    + first_relevant_ptr * core::mem::size_of::<BlockPtr>() as u64;
            let ptrs_to_read = (last_relevant_ptr - first_relevant_ptr + 1) as usize;
            let ptrs_slice = unsafe { core::slice::from_raw_parts(ptr_virt.0 as *const BlockPtr, ptrs_to_read) };

            if current_ptr_level == levels {
                for (ptr, phys) in ptrs_slice.iter().zip(buffer.iter()) {
                    self.partition
                        .write(*ptr as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[*phys])
                        .await;
                }
                for block in current_working_blocks {
                    unsafe { physical_allocator::deallocate_frame(block) };
                }
                break;
            } else {
                let new_working_physical = physical_allocator::allocate_contiguius_high(ptrs_to_read as u64);
                let new_working_blocks = (0..ptrs_to_read)
                    .map(|i| PhysAddr(new_working_physical.0 + i as u64 * 4096))
                    .collect::<Vec<_>>();

                self.read_locked_pointers(ptrs_slice, &new_working_blocks).await;

                for block in current_working_blocks {
                    unsafe { physical_allocator::deallocate_frame(block) };
                }
                current_working_blocks = new_working_blocks;
                current_ptr_level += 1;
            }
        }

        Ok((last_block_to_write - first_block_to_write) * 4096)
    }
}
