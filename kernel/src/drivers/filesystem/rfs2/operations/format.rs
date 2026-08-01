use std::{Box, error::ErrorCode, println};

use crate::{
    drivers::filesystem::rfs2::{
        BLOCK_SIZE_SECTORS, GROUP_SIZE_BLOCKS, InodeIndex, Rfs2, SuperBlock, WorkingBlock, btree::BTreeNode,
        operations::InodeInfo,
    },
    vfs::{InodeTypeAndPerms, ROOT_INODE_INDEX},
};

impl Rfs2 {
    #[heap_future::heap_future]
    pub async fn format(&mut self) -> Result<(), ErrorCode> {
        let whole_blocks = self.partition.partition.size_sectors / BLOCK_SIZE_SECTORS;
        if whole_blocks < 5 {
            return Err(ErrorCode::InsufficientResources);
        }
        let whole_groups = whole_blocks / GROUP_SIZE_BLOCKS;
        let last_group_blocks = whole_blocks % GROUP_SIZE_BLOCKS;

        let mut working_block = WorkingBlock::new();
        *working_block.get_as_mut::<[u8; 4096]>() = [0; 4096];

        println!("whole blocks: {}", whole_blocks);

        //----------clear disk----------
        for i in 0..whole_blocks {
            self.partition
                .write(i * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys.0])
                .await;
        }

        println!("disk cleared");

        //----------initialize free block tables----------

        working_block.get_as_mut::<[u8; 4096]>()[0] = 1;
        for i in 0..whole_groups {
            self.partition
                .write(
                    i * GROUP_SIZE_BLOCKS * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[working_block.phys.0],
                )
                .await;
        }

        if last_group_blocks > 0 {
            let invalid_blocks = GROUP_SIZE_BLOCKS - last_group_blocks;
            let whole_bytes = invalid_blocks / 8;
            let remaining_bits = invalid_blocks % 8;
            let arr = working_block.get_as_mut::<[u8; 4096]>();
            for i in 0..whole_bytes {
                let index = arr.len() - 1 - i;
                arr[index] = 0xFF;
            }
            if remaining_bits > 0 {
                let index = arr.len() - 1 - whole_bytes;
                arr[index] = (1 << remaining_bits) - 1;
            }
            self.partition
                .write(
                    whole_groups * GROUP_SIZE_BLOCKS * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[working_block.phys.0],
                )
                .await;
        }

        println!("blocks: {}", whole_blocks);

        //----------initialize superblock----------

        let mut superblock_block = WorkingBlock::new();
        let mut superblock = SuperBlock {
            inode_mask_ptr: 3,
            inode_tree_root_ptr: 2,
            fs_size: whole_blocks as u64,
            root_inode_index: ROOT_INODE_INDEX as u32,
            checksum: 0,
        };
        superblock.calculate_checksum();
        self.superblock.set(superblock);
        println!("superblock: {:#?}", self.superblock.get());

        *superblock_block.get_as_mut::<SuperBlock>() = superblock;

        for i in (0..(whole_groups + 1)).step_by(64) {
            let block = i * GROUP_SIZE_BLOCKS;
            if block >= whole_blocks {
                break;
            }

            self.partition
                .read(block * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys.0])
                .await;

            working_block.get_as_mut::<[u8; 4096]>()[0] |= 3; //first 2 marked

            self.partition
                .write(block * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys.0])
                .await;
            self.partition
                .write(
                    (block + 1) * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[superblock_block.phys.0],
                )
                .await;
        }
        superblock_block.forget_mem_binding();

        //----------initialize first bitmask----------
        working_block.get_as_mut::<[u8; 4096]>()[0] = 0b11111;
        self.partition.write(0, BLOCK_SIZE_SECTORS, &[working_block.phys.0]).await;

        //----------initialize inode tree root----------
        let node = working_block.get_as_mut::<BTreeNode>();
        node.initialize_root(ROOT_INODE_INDEX as InodeIndex, 4);
        self.partition
            .write(2 * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys.0])
            .await;

        //----------initialize inode bitmask----------
        *working_block.get_as_mut::<[u8; 4096]>() = [0; 4096];
        const _: () = {
            assert!(
                ROOT_INODE_INDEX < 8,
                "ROOT_INODE_INDEX must be less than 8 to fit in the first byte of the inode bitmask"
            );
        };
        let byte = (2 << ROOT_INODE_INDEX) - 1;
        working_block.get_as_mut::<[u8; 4096]>()[0] = byte;
        self.partition
            .write(3 * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys.0])
            .await;

        //----------initialize root inode----------
        let inode_info = working_block.get_as_mut::<InodeInfo>();

        let since_epoch = std::time::Instant::now().duration_since(std::time::UNIX_EPOCH).as_secs();

        *inode_info = InodeInfo {
            size: 0,
            levels: 0,
            type_flags: InodeTypeAndPerms::new_dir(0o755),
            owner_uid: 0,
            owner_gid: 0,
            link_count: 1,
            creation_seconds_since_epoch: since_epoch,
            modification_seconds_since_epoch: since_epoch,
            stat_change_seconds_since_epoch: since_epoch,
        };
        self.partition
            .write(4 * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[working_block.phys.0])
            .await;

        working_block.forget_mem_binding();

        Ok(())
    }
}
