use crate::memory::addresses::*;
use core::cell::Cell;
use std::boxed::Box;
use std::{
    println, r_lock_w_info,
    sync::{arc::Arc, async_lock::AsyncSpinlock, rw_lock::RWSpinlock},
    vec::Vec,
    w_lock_w_info,
};

use uuid::Uuid;

use crate::{
    drivers::{
        block_device::disk::MountedPartition,
        filesystem::rfs2::{bitmask::INODES_PER_BITMASK, superblock::SuperBlock},
    },
    memory::physical_allocator,
    vfs::{FileSystem, FileSystemFactory},
};

mod bitmask;
mod btree;
mod operations;
mod superblock;

const BLOCK_SIZE_SECTORS: usize = 8;
const GROUP_SIZE_BLOCKS: usize = 4096 * 8;

type InodeIndex = u32;
type BlockPtr = u64;

struct WorkingBlock {
    pub virt: VirtAddr,
    pub phys: OwnedPhysAddr,
    pub disk_block: Option<u64>,
    pub changed: bool,
}

impl WorkingBlock {
    fn new() -> Self {
        let phys = physical_allocator::allocate();
        let virt = (&phys).into();
        //no need for UC because of x86 cache coherency
        Self {
            virt,
            phys,
            disk_block: None,
            changed: false,
        }
    }

    pub fn get_as<T: 'static>(&self) -> &T {
        assert!(size_of::<T>() <= 4096);

        unsafe { get_at_addr(self.virt) }
    }

    pub fn get_as_mut<T: 'static>(&mut self) -> &mut T {
        assert!(size_of::<T>() <= 4096);
        self.changed = true;
        unsafe { get_at_addr(self.virt) }
    }

    fn assign_to_disk_block(&mut self, block: u64, changed: bool) {
        self.disk_block = Some(block);
        self.changed = changed;
    }

    async fn write_and_dealloc(self, rfs: &Rfs2) {
        if let Some(block) = self.disk_block
            && self.changed
        {
            rfs.partition
                .write(block as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[self.phys.0])
                .await;
        }
        self.dealloc();
    }

    async fn get_disk_block(rfs: &Rfs2) -> Self {
        let block = rfs.allocate_block().await;
        let mut working_block = Self::new();
        working_block.assign_to_disk_block(block, false);
        working_block
    }

    fn forget_mem_binding(self) {
        if self.disk_block.is_some() && self.changed {
            panic!("cannot forget mem binding of a block that is bound to disk");
        }
        self.dealloc();
    }

    fn dealloc(mut self) {
        let phys = core::mem::replace(&mut self.phys, OwnedPhysAddr(PhysAddr(0)));
        drop(phys);
        core::mem::forget(self);
    }
}

impl Drop for WorkingBlock {
    fn drop(&mut self) {
        panic!("working block was not saved and forgotten");
    }
}

pub(super) fn init_rfs2() {
    crate::vfs::register_filesystem_driver_factory(Arc::new(Rfs2Factory));
}

pub struct Rfs2Factory;

impl Rfs2Factory {
    pub const UUID: Uuid = Uuid::from_u128(0x2477786763f94f0391447b0cad53daad);
}

#[async_trait::async_trait]
impl FileSystemFactory for Rfs2Factory {
    async fn mount(&self, partition: MountedPartition) -> Arc<dyn FileSystem + Send> {
        Arc::new(Rfs2::new(partition).await)
    }

    fn uuid(&self) -> Uuid {
        Rfs2Factory::UUID
    }

    fn name(&self) -> &str {
        "RFS2"
    }
}

#[derive(Debug)]
struct Rfs2 {
    superblock: Cell<SuperBlock>,
    update_superblock_lock: AsyncSpinlock<()>,

    inode_lock: AsyncSpinlock<()>,

    file_locks: RWSpinlock<Vec<(InodeIndex, Arc<AsyncSpinlock<()>>)>>,

    block_alloc_lock: AsyncSpinlock<()>,

    //write once
    groups: u32,
    //write once
    blocks: u32,

    //write once
    partition: MountedPartition,
}

unsafe impl Sync for Rfs2 {}

impl Rfs2 {
    async fn new(partition: MountedPartition) -> Self {
        let blocks = partition.partition.size_sectors as u32 / BLOCK_SIZE_SECTORS as u32;
        let groups = blocks.div_ceil(GROUP_SIZE_BLOCKS as u32);

        let working_block = WorkingBlock::new();
        partition.read(BLOCK_SIZE_SECTORS, 1, &[working_block.phys.0]).await;

        let superblock = *working_block.get_as::<SuperBlock>();
        working_block.forget_mem_binding();

        #[allow(clippy::let_and_return)]
        #[allow(unused_mut)]
        let mut fs = Self {
            superblock: Cell::new(superblock),
            update_superblock_lock: AsyncSpinlock::new(()),
            inode_lock: AsyncSpinlock::new(()),
            file_locks: RWSpinlock::new(Vec::new()),
            block_alloc_lock: AsyncSpinlock::new(()),
            groups,
            blocks,
            partition,
        };

        // println!("formatting rfs2 partition");
        // fs.format().await.expect("failed to format rfs2 filesystem");
        fs
    }

    async fn update_superblock(&self, mut superblock: SuperBlock) {
        superblock.calculate_checksum();

        let lock = self.update_superblock_lock.lock().await;
        self.superblock.set(superblock);
        let mut block = WorkingBlock::new();
        *block.get_as_mut::<SuperBlock>() = superblock;

        for i in (0..(self.groups - 1)).step_by(64) {
            let block_index = 1 + i as usize * GROUP_SIZE_BLOCKS;
            self.partition
                .write(block_index * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
                .await;
        }
        if self.groups % 64 == 1 && //last group is 64 after previous group
            (self.groups as usize - 1) * GROUP_SIZE_BLOCKS < self.blocks as usize
        {
            let block_index = 1 + (self.groups - 1) as usize * GROUP_SIZE_BLOCKS;
            self.partition
                .write(block_index * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
                .await;
        }
        drop(lock);
        block.forget_mem_binding();
    }

    async fn get_superblock(&self) -> SuperBlock {
        let lock = self.update_superblock_lock.lock().await;
        let res = self.superblock.get();
        drop(lock);
        res
    }

    async fn allocate_block(&self) -> BlockPtr {
        let lock = self.block_alloc_lock.lock().await;
        let res = self.allocate_block_locked().await;
        drop(lock);
        res
    }

    async fn allocate_block_locked(&self) -> BlockPtr {
        let mut block = WorkingBlock::new();
        for i in 0..self.groups {
            let block_index = i as usize * GROUP_SIZE_BLOCKS;
            self.partition
                .read(block_index * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
                .await;
            let bitmask = block.get_as_mut::<bitmask::BlockBitmask>();
            if let Some(empty) = bitmask.find_empty() {
                bitmask.set(empty);
                block.assign_to_disk_block(block_index as u64, true);
                block.write_and_dealloc(self).await;
                return (i as u64 * GROUP_SIZE_BLOCKS as u64 + empty as u64) as BlockPtr;
            }
        }
        panic!("disk is full");
    }

    async fn release_block(&self, block: BlockPtr) {
        let lock = self.block_alloc_lock.lock().await;
        self.release_block_locked(block).await;
        drop(lock);
    }

    async fn release_block_locked(&self, block: BlockPtr) {
        if block.is_multiple_of(GROUP_SIZE_BLOCKS as u64) {
            println!(level:error, "refusing to free inode bitmask");
        }
        let group = block / GROUP_SIZE_BLOCKS as u64;
        let block_index = block % GROUP_SIZE_BLOCKS as u64;
        if group & 64 == 0 && block_index == 1 {
            println!(level:error, "refusing to free superblock");
            return;
        }

        let mut block = WorkingBlock::new();
        let bitmask_block_index = group as usize * GROUP_SIZE_BLOCKS;
        self.partition
            .read(bitmask_block_index * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
            .await;
        let bitmask = block.get_as_mut::<bitmask::BlockBitmask>();
        bitmask.clear(block_index as usize);
        block.assign_to_disk_block(bitmask_block_index as u64, true);
        block.write_and_dealloc(self).await;
    }

    #[heap_future::heap_future]
    pub async fn allocate_inode(&self) -> InodeIndex {
        let mut block = WorkingBlock::new();
        let mut iteration_index = 0;
        let mut superblock = self.get_superblock().await;
        if superblock.inode_mask_ptr == 0 {
            let inode_bitmask_block = self.allocate_block().await;
            let inode_bitmask_block_data = block.get_as_mut::<bitmask::InodeBtmask>();
            *inode_bitmask_block_data = bitmask::InodeBtmask::new();
            block.assign_to_disk_block(inode_bitmask_block, true);
            superblock.inode_mask_ptr = inode_bitmask_block;
            self.update_superblock(superblock).await;
        } else {
            self.partition
                .read(
                    superblock.inode_mask_ptr as usize * BLOCK_SIZE_SECTORS,
                    BLOCK_SIZE_SECTORS,
                    &[block.phys.0],
                )
                .await;
            block.assign_to_disk_block(superblock.inode_mask_ptr, false);
        }

        loop {
            let bitmask = block.get_as::<bitmask::InodeBtmask>();
            let empty = bitmask.find_empty();
            let Some(empty) = empty else {
                let next_ptr = bitmask.get_ptr();
                if next_ptr == 0 {
                    let inode_bitmask_block = self.allocate_block().await;

                    block.get_as_mut::<bitmask::InodeBtmask>().set_ptr(inode_bitmask_block);
                    self.partition
                        .write(
                            block.disk_block.expect("is assigned") as usize * BLOCK_SIZE_SECTORS,
                            BLOCK_SIZE_SECTORS,
                            &[block.phys.0],
                        )
                        .await;

                    let inode_bitmask_block_data = block.get_as_mut::<bitmask::InodeBtmask>();
                    *inode_bitmask_block_data = bitmask::InodeBtmask::new();
                    block.assign_to_disk_block(inode_bitmask_block, true);
                    superblock.inode_mask_ptr = inode_bitmask_block;
                    self.update_superblock(superblock).await;
                } else {
                    if block.changed {
                        self.partition
                            .write(
                                block.disk_block.expect("is assigned") as usize * BLOCK_SIZE_SECTORS,
                                BLOCK_SIZE_SECTORS,
                                &[block.phys.0],
                            )
                            .await;
                    }

                    self.partition
                        .read(next_ptr as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
                        .await;
                    block.assign_to_disk_block(next_ptr, false);
                }
                iteration_index += 1;
                continue;
            };

            block.get_as_mut::<bitmask::InodeBtmask>().set(empty);
            block.changed = true;
            block.write_and_dealloc(self).await;

            return (iteration_index * INODES_PER_BITMASK + empty) as InodeIndex;
        }
    }

    pub async fn release_inode(&self, index: InodeIndex) {
        let bitmask_index = index as usize / INODES_PER_BITMASK;
        let in_bitmask_index = index as usize % INODES_PER_BITMASK;

        let mut block = WorkingBlock::new();
        let superblock = self.get_superblock().await;

        let mut current_ptr = superblock.inode_mask_ptr;
        for _ in 0..bitmask_index {
            if current_ptr == 0 {
                println!(level:error, "inode index {} is out of bounds", index);
                return;
            }
            self.partition
                .read(current_ptr as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
                .await;
            let bitmask = block.get_as::<bitmask::InodeBtmask>();
            current_ptr = bitmask.get_ptr();
        }
        if current_ptr == 0 {
            println!(level:error, "inode index {} is out of bounds", index);
            return;
        }

        self.partition
            .read(current_ptr as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
            .await;
        let bitmask = block.get_as_mut::<bitmask::InodeBtmask>();
        bitmask.clear(in_bitmask_index);
        block.assign_to_disk_block(current_ptr, true);
        block.write_and_dealloc(self).await;
    }

    pub fn get_file_lock(&self, inode: InodeIndex) -> Arc<AsyncSpinlock<()>> {
        let vec = r_lock_w_info!(self.file_locks);
        if let Some(item) = vec.iter().find(|i| i.0 == inode) {
            return item.1.clone();
        }
        let new_lock = Arc::new(AsyncSpinlock::new(()));
        drop(vec);
        let mut vec = w_lock_w_info!(self.file_locks);
        vec.push((inode, new_lock.clone()));
        new_lock
    }

    pub async fn get_disk_block(&self, disk_block: BlockPtr) -> WorkingBlock {
        let mut block = WorkingBlock::new();
        self.partition
            .read(disk_block as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
            .await;
        block.disk_block = Some(disk_block);
        block
    }

    pub async fn write_disk_block(&self, block: &mut WorkingBlock) {
        if !block.changed {
            return;
        }
        if let Some(disk_block) = block.disk_block {
            self.partition
                .write(disk_block as usize * BLOCK_SIZE_SECTORS, BLOCK_SIZE_SECTORS, &[block.phys.0])
                .await;
            block.changed = false;
        } else {
            panic!("cannot write a block that is not assigned to disk");
        }
    }
}
