use std::{boxed::Box, mem_utils::memset_physical_addr};

use crate::{
    drivers::filesystem::rfs2::{
        BLOCK_SIZE_SECTORS, BlockPtr, Rfs2, WorkingBlock,
        operations::{PTRS_IN_ROOT, PTRS_PER_BLOCK},
    },
    memory::physical_allocator,
};

impl Rfs2 {
    pub(super) async fn increase_file_size(&self, file_root: BlockPtr, new_size: usize) {
        if new_size < (BLOCK_SIZE_SECTORS - 1) * 512 {
            return; //small file, no increase
        }

        let mut file_info = self.get_file_info(file_root).await;
        let current_size = file_info.size;
        let current_blocks = (current_size as usize).div_ceil(BLOCK_SIZE_SECTORS * 512);
        let current_levels = file_info.levels;

        let needed_blocks = new_size.div_ceil(BLOCK_SIZE_SECTORS * 512);

        if needed_blocks <= current_blocks {
            return;
        }

        let needed_levels = {
            let mut needed_blocks = needed_blocks;
            let mut needed_levles = 1;
            while needed_blocks > PTRS_IN_ROOT {
                needed_levles += 1;
                needed_blocks = needed_blocks.div_ceil(PTRS_PER_BLOCK);
            }
            needed_levles
        };

        self.increase_file_depth(file_root, needed_levels as i8 - current_levels as i8)
            .await;
        let current_levels = current_levels.max(needed_levels);

        let mut working_block = WorkingBlock::new();
        self.partition
            .read(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                BLOCK_SIZE_SECTORS - 1,
                &[working_block.phys],
            )
            .await;
        let pointers = working_block.get_as_mut::<[BlockPtr; PTRS_PER_BLOCK]>();
        let last_ptr = (pointers.iter().find(|ptr| **ptr == 0).cloned().unwrap_or(512) as usize).saturating_sub(1);
        let mut left_to_allocate = needed_blocks - current_blocks;
        for i in last_ptr..PTRS_IN_ROOT {
            let mut is_new_block = false;
            if pointers[i] == 0 {
                let new_block = self.allocate_block().await;
                pointers[i] = new_block;
                is_new_block = true;
            }
            left_to_allocate -= self
                .increase_file_size_recursively(pointers[i], current_levels - 1, left_to_allocate, is_new_block)
                .await;
            if left_to_allocate == 0 {
                break;
            }
        }

        self.partition
            .write(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                BLOCK_SIZE_SECTORS - 1,
                &[working_block.phys],
            )
            .await;

        file_info.size = new_size as u64;
        file_info.levels = current_levels;
        self.set_file_info(file_root, file_info).await;
        working_block.forget_mem_binding();
    }

    async fn increase_file_size_recursively(
        &self,
        working_block_ptr: BlockPtr,
        current_level: u8,
        left_blocks_to_allocate: usize,
        new_block: bool,
    ) -> usize {
        if current_level == 0 {
            if !new_block {
                return 0;
            }
            let frame = physical_allocator::allocate_frame();
            unsafe { memset_physical_addr(frame, 0, 4096) };
            self.partition
                .write(working_block_ptr as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[frame])
                .await;
            unsafe { physical_allocator::deallocate_frame(frame) };

            return 1; //final block, does not contain pointers
        }

        let mut working_block = WorkingBlock::new();

        if !new_block {
            self.partition
                .read(
                    working_block_ptr as usize * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[working_block.phys],
                )
                .await;
        } else {
            unsafe { memset_physical_addr(working_block.phys, 0, 4096) };
        }

        let pointers = working_block.get_as_mut::<[BlockPtr; PTRS_PER_BLOCK]>();
        let last_ptr = (pointers.iter().find(|ptr| **ptr == 0).cloned().unwrap_or(512) as usize).saturating_sub(1);
        let mut left_to_allocate = left_blocks_to_allocate;
        let mut allocated = 0;

        for i in last_ptr..PTRS_PER_BLOCK {
            let mut is_new_block = false;
            if pointers[i] == 0 {
                let new_block = self.allocate_block().await;
                pointers[i] = new_block;
                is_new_block = true;
            }
            let allocated_here =
                Box::pin(self.increase_file_size_recursively(pointers[i], current_level - 1, left_to_allocate, is_new_block))
                    .await;
            left_to_allocate = left_to_allocate.saturating_sub(allocated_here);
            allocated += allocated_here;
            if left_to_allocate == 0 {
                break;
            }
        }

        allocated
    }

    async fn increase_file_depth(&self, file_root: BlockPtr, increase_by: i8) {
        if increase_by <= 0 {
            return;
        }

        let mut working_block = WorkingBlock::new();
        *working_block.get_as_mut::<[u8; 4096]>() = [0_u8; 4096];

        self.partition
            .read(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                BLOCK_SIZE_SECTORS - 1,
                &[working_block.phys],
            )
            .await;

        for _ in 0..increase_by {
            let new_block = self.allocate_block().await;
            self.partition
                .write(
                    new_block as usize * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[working_block.phys],
                )
                .await;

            *working_block.get_as_mut::<[u8; 4096]>() = [0_u8; 4096];
            *working_block.get_as_mut::<BlockPtr>() = new_block;
        }

        self.partition
            .write(
                file_root as usize * BLOCK_SIZE_SECTORS,
                BLOCK_SIZE_SECTORS,
                &[working_block.phys],
            )
            .await;

        working_block.forget_mem_binding();
    }
}
