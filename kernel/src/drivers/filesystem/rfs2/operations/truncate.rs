use std::boxed::Box;

use crate::drivers::filesystem::rfs2::{
    BLOCK_SIZE_SECTORS, BlockPtr, Rfs2, WorkingBlock,
    operations::{PTRS_IN_ROOT, PTRS_PER_BLOCK},
};

impl Rfs2 {
    pub(super) async fn truncate_locked(&self, file_root: u64, new_size_bytes: usize) {
        let mut file_info = self.get_file_info(file_root).await;
        let current_size = file_info.size as usize;
        let current_blocks = current_size.div_ceil(BLOCK_SIZE_SECTORS * 512);
        let current_levels = file_info.levels;

        let needed_blocks = new_size_bytes.div_ceil(BLOCK_SIZE_SECTORS * 512);

        if needed_blocks >= current_blocks {
            return; //no truncation needed
        }

        let needed_levels = if new_size_bytes <= (BLOCK_SIZE_SECTORS - 1) * 512 {
            0
        } else {
            let mut needed_blocks = needed_blocks;
            let mut needed_levles = 1;
            while needed_blocks > PTRS_IN_ROOT {
                needed_levles += 1;
                needed_blocks = needed_blocks.div_ceil(PTRS_PER_BLOCK);
            }
            needed_levles
        };

        self.decrease_file_depth(file_root, current_levels as i8 - needed_levels as i8)
            .await;
        let current_levels = current_levels.min(needed_levels);

        if current_levels == 0 {
            file_info.levels = 0;
            file_info.size = new_size_bytes as u64;
            self.set_file_info(file_root, file_info).await;
            return;
        }

        let mut working_block = WorkingBlock::new();
        self.partition
            .read(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                BLOCK_SIZE_SECTORS - 1,
                &[working_block.phys],
            )
            .await;

        let pointers = working_block.get_as_mut::<[BlockPtr; PTRS_PER_BLOCK]>();
        let ptr_blocks = PTRS_PER_BLOCK.pow(current_levels as u32 - 1);
        let mut file_scanned_blocks = 0;
        for i in 0..PTRS_PER_BLOCK {
            if pointers[i] == 0 {
                break;
            }
            if file_scanned_blocks + ptr_blocks < needed_blocks {
                file_scanned_blocks += ptr_blocks;
                continue;
            }

            self.decrease_file_size_recursively(pointers[i], current_levels - 1, needed_blocks - file_scanned_blocks)
                .await;
            file_scanned_blocks += ptr_blocks;
            if file_scanned_blocks >= needed_blocks {
                for j in i..PTRS_PER_BLOCK {
                    if pointers[j] == 0 {
                        break;
                    }
                    self.release_block(pointers[j]).await;
                    pointers[j] = 0;
                }
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

        file_info.size = new_size_bytes as u64;
        file_info.levels = current_levels;
        self.set_file_info(file_root, file_info).await;
        working_block.forget_mem_binding();
    }

    async fn decrease_file_size_recursively(&self, block_ptr: BlockPtr, current_level: u8, left_to_keep: usize) {
        if current_level == 0 {
            return; //final block, does not contain pointers
        }

        let mut working_block = WorkingBlock::new();
        self.partition
            .read(
                block_ptr as usize * BLOCK_SIZE_SECTORS,
                BLOCK_SIZE_SECTORS,
                &[working_block.phys],
            )
            .await;

        let pointers = working_block.get_as_mut::<[BlockPtr; PTRS_PER_BLOCK]>();
        let ptr_blocks = PTRS_PER_BLOCK.pow(current_level as u32 - 1);
        let mut scanned_blocks = 0;
        for i in 0..PTRS_PER_BLOCK {
            if pointers[i] == 0 {
                break;
            }
            if scanned_blocks + ptr_blocks < left_to_keep {
                scanned_blocks += ptr_blocks;
                continue;
            }

            Box::pin(self.decrease_file_size_recursively(pointers[i], current_level - 1, left_to_keep - scanned_blocks))
                .await;
            scanned_blocks += ptr_blocks;
            if scanned_blocks >= left_to_keep {
                for j in i..PTRS_PER_BLOCK {
                    if pointers[j] == 0 {
                        break;
                    }
                    self.release_block(pointers[j]).await;
                    pointers[j] = 0;
                }
                break;
            }
        }

        self.partition
            .write(
                block_ptr as usize * BLOCK_SIZE_SECTORS,
                BLOCK_SIZE_SECTORS,
                &[working_block.phys],
            )
            .await;
    }

    async fn decrease_file_depth(&self, file_root: u64, decrease_by: i8) {
        if decrease_by <= 0 {
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

        for _ in 0..decrease_by {
            let first_ptr = *working_block.get_as_mut::<BlockPtr>();
            self.partition
                .read(
                    first_ptr as usize * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[working_block.phys],
                )
                .await;
            self.release_block(first_ptr).await;
        }

        self.partition
            .write(
                file_root as usize * BLOCK_SIZE_SECTORS + 1,
                BLOCK_SIZE_SECTORS - 1,
                &[working_block.phys],
            )
            .await;
    }
}
