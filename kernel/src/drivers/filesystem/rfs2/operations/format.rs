use std::error::ErrorCode;

use crate::drivers::filesystem::rfs2::{BLOCK_SIZE_SECTORS, GROUP_SIZE_BLOCKS, Rfs2, WorkingBlock};


impl Rfs2 {
    pub async fn format(&mut self) -> Result<(), ErrorCode> {
        let whole_blocks = self.partition.partition.size_sectors / BLOCK_SIZE_SECTORS;
        if whole_blocks < 5 {
            return Err(ErrorCode::InsufficientResources);
        }
        let whole_groups = whole_blocks / GROUP_SIZE_BLOCKS;
        let last_group_blocks = whole_blocks % GROUP_SIZE_BLOCKS;

        let mut working_block = WorkingBlock::new();
        *working_block.get_as_mut::<[u8; 4096]>() = [0; 4096];

        //----------initialize free block tables----------

        working_block.get_as_mut::<[u8; 4096]>()[0] = 1;
        for i in 0..whole_groups {
            self.partition.write(i * GROUP_SIZE_BLOCKS * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys]).await;
        }

        todo!()
    }
}
