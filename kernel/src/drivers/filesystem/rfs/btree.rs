use std::{
    boxed::Box,
    mem_utils::{PhysAddr, VirtAddr}, println,
};

use super::{AllocatedBlock, BLOCK_SIZE_SECTORS, MountedPartition, Rfs};
use crate::memory::{
    PAGE_TREE_ALLOCATOR,
    paging,
    physical_allocator,
};

///Takes up exactly 1 block or physical frame
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BtreeNode {
    keys: [Key; 341],
    children: [u32; 342],
}

impl BtreeNode {
    pub async fn read_from_disk(partition: &MountedPartition, block: u32) -> AllocatedBlock {
        let sector = block as usize * BLOCK_SIZE_SECTORS;

        let phys_ptr = physical_allocator::allocate_frame();
        let virt_ptr = unsafe { PAGE_TREE_ALLOCATOR.allocate(Some(phys_ptr), false) };
        unsafe {
            PAGE_TREE_ALLOCATOR
                .get_page_table_entry_mut(virt_ptr)
                .expect("page entry must exist after allocation")
                .set_pat(paging::LiminePat::UC);
        }
        partition.read(sector, BLOCK_SIZE_SECTORS, &[phys_ptr]).await;
        AllocatedBlock::new(phys_ptr, virt_ptr)
    }

    ///set modified to false
    pub async fn write_to_disk(node: PhysAddr, partition: &MountedPartition, block: u32) {
        let sector = block as usize * BLOCK_SIZE_SECTORS;

        partition.write(sector, BLOCK_SIZE_SECTORS, &[node]).await;
    }

    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> AllocatedBlock {
        let phys_ptr = physical_allocator::allocate_frame();
        let virt_ptr = unsafe { PAGE_TREE_ALLOCATOR.allocate(Some(phys_ptr), false) };
        unsafe {
            std::mem_utils::memset_virtual_addr(virt_ptr, 0, 4096);
        }
        AllocatedBlock::new(phys_ptr, virt_ptr)
    }

    fn get_key(node: VirtAddr, index: usize) -> Key {
        assert!(index < 341);
        unsafe { (node.0 as *const Key).add(index).read_volatile() }
    }

    pub fn set_key(node: VirtAddr, index: usize, key: Key) {
        assert!(index < 341);
        unsafe {
            (node.0 as *mut Key).add(index).write_volatile(key);
        }
    }

    fn get_child(node: VirtAddr, index: usize) -> u32 {
        assert!(index < 342);
        unsafe { (node.0 as *const u32).byte_add(0xAA8).add(index).read_volatile() }
    }

    fn set_child(node: VirtAddr, index: usize, child: u32) {
        assert!(index < 342);
        unsafe {
            (node.0 as *mut u32).byte_add(0xAA8).add(index).write_volatile(child);
        }
    }

    ///Returns the block intex on the disk (each has 8 sectors) where the inode is stored
    pub async fn find_inode_block(node: VirtAddr, key_index: u32, fs_data: &Rfs) -> Option<u32> {
        for i in 0..341 {
            let key = Self::get_key(node, i);
            if key.index == 0 {
                if Self::get_child(node, i) == 0 {
                    return None;
                }
                let child_block = Self::get_child(node, i);
                let child_node = unsafe { fs_data.get_node(child_block).await.1.virt };
                return Box::pin(BtreeNode::find_inode_block(child_node, key_index, fs_data)).await;
            }
            if key.index == key_index {
                return Some(key.inode_block);
            }
            if key.index > key_index {
                if Self::get_child(node, i) == 0 {
                    return None;
                }
                let child_block = Self::get_child(node, i);
                let child_node = unsafe { fs_data.get_node(child_block).await.1.virt };
                return Box::pin(BtreeNode::find_inode_block(child_node, key_index, fs_data)).await;
            }
        }
        if Self::get_child(node, 341) == 0 {
            return None;
        }
        let child_block = Self::get_child(node, 341);
        let child_node = unsafe { fs_data.get_node(child_block).await.1.virt };
        Box::pin(BtreeNode::find_inode_block(child_node, key_index, fs_data)).await
    }

    //returns a new root node if the root was split
    pub async fn insert_key_root(node: VirtAddr, block: u32, key: Key, fs_data: &Rfs) -> Option<u32> {
        //print all params
        println!("insert_key_root: node={:?}, block={}, key={:?}", node, block, key);
        let is_leaf = Self::get_child(node, 0) == 0;
        let is_full = Self::get_key(node, 340).index != 0;
        println!("is_leaf={}, is_full={}", is_leaf, is_full);

        if is_leaf {
            if is_full {
                let new_root_block = Self::split_root(node, block, fs_data).await;
                let new_root_node = unsafe { fs_data.get_node(new_root_block).await.1.virt };
                if key.index < BtreeNode::get_key(new_root_node, 0).index {
                    BtreeNode::insert_key_internal(new_root_node, new_root_block, 0, key, fs_data).await;
                } else {
                    BtreeNode::insert_key_internal(new_root_node, new_root_block, block, key, fs_data).await;
                }
                Some(new_root_block)
            } else {
                BtreeNode::insert_non_full(node, block, key, None, fs_data).await;
                None
            }
        } else {
            //find first bigger key index
            for i in 0..341 {
                if key.index < Self::get_key(node, i).index || Self::get_key(node, i).index == 0 {
                    return BtreeNode::insert_into_root_child(node, block, i, key, fs_data).await;
                }
            }
            BtreeNode::insert_into_root_child(node, block, 341, key, fs_data).await
        }
    }

    async fn insert_into_root_child(node: VirtAddr, block: u32, child_index: usize, key: Key, fs_data: &Rfs) -> Option<u32> {
        let is_full = Self::get_key(node, 340).index != 0;
        let child_node_index = Self::get_child(node, child_index);
        let rebalance_result = BtreeNode::insert_key_internal(
            unsafe { fs_data.get_node(child_node_index).await.1.virt },
            child_node_index,
            block,
            key,
            fs_data,
        )
        .await;
        match rebalance_result {
            RebalanceResult::None => None,
            RebalanceResult::Merge(_) => {
                unreachable!("Nodes should not be merged when inserting keys");
            }
            RebalanceResult::Rotate(_) => {
                //child does everything
                None
            }
            RebalanceResult::Split(new_block, new_key) => {
                if is_full {
                    let new_root_block = Self::split_root(node, block, fs_data).await;
                    let new_root_node = unsafe { fs_data.get_node(new_root_block).await.1.virt };
                    if new_key.index < BtreeNode::get_key(new_root_node, 0).index {
                        BtreeNode::insert_key_internal(new_root_node, new_root_block, 0, new_key, fs_data).await;
                    } else {
                        BtreeNode::insert_key_internal(new_root_node, new_root_block, block, new_key, fs_data).await;
                    }
                    Some(new_root_block)
                } else {
                    Self::insert_non_full(node, block, new_key, Some(new_block), fs_data).await;
                    None
                }
            }
        }
    }

    //returns a new root node if the root was merged
    pub async fn delete_key_root(node: VirtAddr, block: u32, key_index: u32, fs_data: &Rfs) -> Option<u32> {
        assert!(key_index > 2); //no deleting null keys, bad block file or root
        let is_leaf = Self::get_child(node, 0) == 0;

        if is_leaf {
            let mut deleted = false;
            for i in 0..341 {
                //will get overwritten, everything else past it will be shifted
                if key_index == Self::get_key(node, i).index {
                    deleted = true;
                }
                if deleted && i < 340 {
                    Self::set_key(node, i, Self::get_key(node, i + 1));
                }
            }
            Self::set_key(node, 340, Key::empty());
            unsafe { fs_data.get_node(block).await.0 = true };
            assert!(deleted);
            return None;
        }

        for i in 0..341 {
            if key_index == Self::get_key(node, i).index {
                //child on the left of the key
                let child_block = Self::get_child(node, i);
                let child_node = unsafe { fs_data.get_node(child_block).await.1.virt };
                let (key, rebalance_result) = BtreeNode::take_biggest_key(child_node, child_block, block, fs_data).await;
                //If the result is a merge, this key has escaped to the merged child. Find and
                //replace it there. No need to worry about additional merges, as the node will be
                //full
                //if it is a rotate, it also disappeared somewhere
                match rebalance_result {
                    RebalanceResult::Merge(direction) => {
                        if matches!(direction, MergeDirection::RightToCurrent) {
                            for i in 0..341 {
                                if BtreeNode::get_key(child_node, i).index == key_index {
                                    BtreeNode::set_key(child_node, i, key);
                                    break;
                                }
                            }
                        } else {
                            BtreeNode::set_key(child_node, i - 1, key);
                        }
                        unsafe { fs_data.get_node(child_block).await.0 = true };

                        if Self::get_key(node, 0).index == 0 {
                            //root is empty, merge
                            let child = Self::get_child(node, 0);
                            fs_data.free_block(block).await;
                            unsafe { fs_data.remove_inode_cache_entry(block) };
                            return Some(child);
                        } else {
                            return None;
                        }
                    }
                    RebalanceResult::Rotate(direction) => {
                        if matches!(direction, RotateDirection::Left) {
                            for i in 160..180 {
                                if BtreeNode::get_key(child_node, i).index == key_index {
                                    BtreeNode::set_key(child_node, i, key);
                                    unsafe { fs_data.get_node(child_block).await.0 = true };
                                    return None;
                                }
                            }
                            unreachable!("Key not found in child");
                        } else {
                            assert!(BtreeNode::get_key(child_node, i).index != key_index);
                            Self::set_key(node, i, key);
                            unsafe { fs_data.get_node(block).await.0 = true };
                            return None;
                        }
                    }

                    RebalanceResult::Split(_, _) => unreachable!("Split should not happen when deleting keys"),
                    RebalanceResult::None => {
                        Self::set_key(node, i, key);
                        unsafe { fs_data.get_node(block).await.0 = true };
                        return None;
                    }
                }
            } else if key_index < Self::get_key(node, i).index || Self::get_key(node, i).index == 0 {
                return BtreeNode::delete_key_root_internal(node, block, key_index, i, fs_data).await;
            }
        }
        BtreeNode::delete_key_root_internal(node, block, key_index, 341, fs_data).await
    }

    async fn delete_key_root_internal(
        node: VirtAddr,
        block: u32,
        key_index: u32,
        child_index: usize,
        fs_data: &Rfs,
    ) -> Option<u32> {
        let child_block = Self::get_child(node, child_index);
        let child_node = unsafe { fs_data.get_node(child_block).await.1.virt };
        let rebalance_result = BtreeNode::delete_key_internal(child_node, child_block, key_index, block, fs_data).await;

        match rebalance_result {
            RebalanceResult::Merge(_) => {
                if Self::get_key(node, 0).index == 0 {
                    //root is empty, merge
                    fs_data.free_block(block).await;
                    let child = BtreeNode::get_child(node, 0);
                    unsafe { fs_data.remove_inode_cache_entry(block) };
                    Some(child)
                } else {
                    None
                }
            }
            RebalanceResult::Rotate(_) => None,
            RebalanceResult::Split(_, _) => unreachable!("Split should not happen when deleting keys"),
            RebalanceResult::None => None,
        }
    }

    async fn delete_key_internal(
        node: VirtAddr,
        block: u32,
        key_index: u32,
        parent_block: u32,
        fs_data: &Rfs,
    ) -> RebalanceResult {
        let is_leaf = Self::get_child(node, 0) == 0;

        if is_leaf {
            let mut deleted = false;
            for i in 0..341 {
                //will get overwritten, everything else past it will be shifted
                if key_index == Self::get_key(node, i).index {
                    deleted = true;
                }
                if deleted && i < 340 {
                    Self::set_key(node, i, Self::get_key(node, i + 1));
                }
            }
            Self::set_key(node, 340, Key::empty());
            unsafe { fs_data.get_node(block).await.0 = true };
            assert!(deleted);
            let needs_rebalance = Self::get_key(node, 169).index == 0;
            if !needs_rebalance {
                return RebalanceResult::None;
            }
            let mut left = false;
            let mut result = BtreeNode::rotate_left_take(node, block, parent_block, fs_data, true).await;
            if !result {
                left = true;
                result = BtreeNode::rotate_right_take(node, block, parent_block, fs_data, true).await;
            }
            if result {
                return RebalanceResult::Rotate(if left { RotateDirection::Left } else { RotateDirection::Right });
            }
            let direction = BtreeNode::merge(node, block, parent_block, fs_data).await;
            return RebalanceResult::Merge(direction);
        }

        for i in 0..341 {
            if key_index == Self::get_key(node, i).index {
                //child on the left of the key
                let child_block = Self::get_child(node, i);
                let child_node = unsafe { fs_data.get_node(child_block).await.1.virt };
                let (key, rebalance_result) = BtreeNode::take_biggest_key(child_node, child_block, block, fs_data).await;
                //If the result is a merge, this key has escaped to the merged child. Find and
                //replace it there. No need to worry about additional merges, as the node will be
                //full
                //if it is a rotate, it also disappeared somewhere
                match rebalance_result {
                    RebalanceResult::Merge(direction) => {
                        if matches!(direction, MergeDirection::RightToCurrent) {
                            for i in 0..341 {
                                if BtreeNode::get_key(child_node, i).index == key_index {
                                    BtreeNode::set_key(child_node, i, key);
                                    break;
                                }
                            }
                        } else {
                            BtreeNode::set_key(child_node, i - 1, key);
                        }
                        unsafe { fs_data.get_node(child_block).await.0 = true };

                        if Self::get_key(node, 169).index == 0 {
                            //Node is too small, fix

                            let mut left = false;
                            let mut result = BtreeNode::rotate_left_take(node, block, parent_block, fs_data, false).await;
                            if !result {
                                left = true;
                                result = BtreeNode::rotate_right_take(node, block, parent_block, fs_data, false).await;
                            }
                            if result {
                                return RebalanceResult::Rotate(if left {
                                    RotateDirection::Left
                                } else {
                                    RotateDirection::Right
                                });
                            }

                            let direction = BtreeNode::merge(node, block, parent_block, fs_data).await;
                            return RebalanceResult::Merge(direction);
                        }
                    }
                    RebalanceResult::Rotate(direction) => {
                        if matches!(direction, RotateDirection::Left) {
                            for i in 160..341 {
                                if Self::get_key(node, i).index == key_index {
                                    Self::set_key(node, i, key);
                                    unsafe { fs_data.get_node(block).await.0 = true };
                                    return RebalanceResult::None;
                                }
                            }
                            unreachable!("Key not found in child");
                        } else {
                            assert!(Self::get_key(node, i).index != key_index);
                            unsafe { fs_data.get_node(block).await.0 = true };
                            BtreeNode::set_key(node, i, key);
                            return RebalanceResult::None;
                        }
                    }

                    RebalanceResult::Split(_, _) => unreachable!("Split should not happen when deleting keys"),
                    RebalanceResult::None => {
                        unsafe { fs_data.get_node(block).await.0 = true };
                        BtreeNode::set_key(node, i, key);
                        return RebalanceResult::None;
                    }
                }
                unreachable!();
            } else if key_index < BtreeNode::get_key(node, i).index {
                return Box::pin(BtreeNode::delete_key_internal(node, block, key_index, i as u32, fs_data)).await;
            }
        }
        Box::pin(BtreeNode::delete_key_internal(node, block, key_index, 341, fs_data)).await
    }

    async fn take_biggest_key(node: VirtAddr, block: u32, parent_block: u32, fs_data: &Rfs) -> (Key, RebalanceResult) {
        //this code is almost identical to the delete_key_internal
        let is_leaf = BtreeNode::get_child(node, 0) == 0;
        if is_leaf {
            let mut index = 0;
            for i in (0..341).rev() {
                if BtreeNode::get_key(node, i).index != 0 {
                    index = i;
                    break;
                }
            }
            if index < 169 {
                assert!(index == 168);
                let key = BtreeNode::get_key(node, index);
                BtreeNode::set_key(node, index, Key::empty());

                //we need to rebalance
                let mut left = false;
                let mut result = BtreeNode::rotate_left_take(node, block, parent_block, fs_data, true).await;
                if !result {
                    left = true;
                    result = BtreeNode::rotate_right_take(node, block, parent_block, fs_data, true).await;
                }
                index += 1;

                if result {
                    let key = BtreeNode::get_key(node, index);
                    BtreeNode::set_key(node, index, Key::empty());
                    return (
                        key,
                        RebalanceResult::Rotate(if left { RotateDirection::Left } else { RotateDirection::Right }),
                    );
                } else {
                    //merge is needed
                    let direction = BtreeNode::merge(node, block, parent_block, fs_data).await;
                    return (key, RebalanceResult::Merge(direction));
                }
            }
            let key = BtreeNode::get_key(node, index);
            unsafe { fs_data.get_node(block).await.0 = true };
            BtreeNode::set_key(node, index, Key::empty());
            return (key, RebalanceResult::None);
        }

        for i in (0..341).rev() {
            if BtreeNode::get_key(node, i).index != 0 {
                let child_index = i + 1;
                return Box::pin(BtreeNode::take_biggest_from_child(
                    node,
                    block,
                    child_index,
                    parent_block,
                    fs_data,
                ))
                .await;
            }
        }
        unreachable!("This should never happen, as the node would have to have 0 children");
    }

    async fn take_biggest_from_child(
        node: VirtAddr,
        block: u32,
        child_index: usize,
        parent_block: u32,
        fs_data: &Rfs,
    ) -> (Key, RebalanceResult) {
        let child_node_block = BtreeNode::get_child(node, child_index);
        let (key, rebalance_result) = BtreeNode::take_biggest_key(
            unsafe { fs_data.get_node(child_node_block).await.1.virt },
            child_node_block,
            block,
            fs_data,
        )
        .await;
        match rebalance_result {
            RebalanceResult::Merge(_direction) => {
                let mut last_key = 0;
                for i in (0..342).rev() {
                    if BtreeNode::get_key(node, i).index != 0 {
                        last_key = i;
                        break;
                    }
                }
                if last_key > 169 {
                    //is still balanced
                    return (key, RebalanceResult::None);
                }
                //rotate
                let mut result = BtreeNode::rotate_left_give(block, parent_block, fs_data, false).await;
                if !result {
                    result = BtreeNode::rotate_right_give(block, parent_block, fs_data, false).await;
                }
                if result {
                    return (key, RebalanceResult::None);
                }

                let direction = BtreeNode::merge(node, block, parent_block, fs_data).await;
                (key, RebalanceResult::Merge(direction))
            }
            RebalanceResult::Rotate(_) => (key, RebalanceResult::None),
            RebalanceResult::Split(_, _) => unreachable!("Split should not happen when taking keys"),
            RebalanceResult::None => (key, RebalanceResult::None),
        }
    }

    async fn merge(node: VirtAddr, block: u32, parent_block: u32, fs_data: &Rfs) -> MergeDirection {
        let parent = unsafe { fs_data.get_node(parent_block).await.1.virt };
        let self_index = unsafe { &*(parent.0 as *const BtreeNode) }
            .children
            .iter()
            .position(|&x| x == block)
            .expect("block must be a child of parent");
        let (left_node, right_node, separator, direction, right_block, left_block);
        if self_index == 0 {
            left_block = block;
            left_node = node;
            right_block = BtreeNode::get_child(parent, self_index + 1);
            right_node = unsafe { fs_data.get_node(right_block).await.1.virt };
            separator = BtreeNode::get_key(parent, self_index);
            direction = MergeDirection::RightToCurrent;
        } else {
            left_block = BtreeNode::get_child(parent, self_index - 1);
            left_node = unsafe { fs_data.get_node(left_block).await.1.virt };
            right_node = node;
            separator = BtreeNode::get_key(parent, self_index - 1);
            direction = MergeDirection::CurrentToLeft;
            right_block = block;
        }

        let mut key_index = 0;
        //it's somewhere in the middle idk exactly
        for i in (160..180).rev() {
            if BtreeNode::get_key(left_node, i).index != 0 {
                key_index = i + 1;
                break;
            }
        }
        assert!(key_index != 0);

        BtreeNode::set_key(left_node, key_index, separator);
        key_index += 1;

        let mut right_ptr = 0;
        let mut right_key = BtreeNode::get_key(right_node, right_ptr);
        while right_key.index != 0 {
            BtreeNode::set_key(left_node, key_index, right_key);
            BtreeNode::set_key(right_node, right_ptr, Key::empty());
            BtreeNode::set_child(left_node, key_index, BtreeNode::get_child(right_node, right_ptr));
            BtreeNode::set_child(right_node, right_ptr, 0);
            key_index += 1;
            right_ptr += 1;
            right_key = BtreeNode::get_key(right_node, right_ptr);
        }

        let shift_ptr = if self_index == 0 { 0 } else { self_index - 1 };
        for i in shift_ptr..340 {
            BtreeNode::set_key(parent, i, BtreeNode::get_key(parent, i + 1));
            BtreeNode::set_child(parent, i + 1, BtreeNode::get_child(parent, i + 2));
        }

        BtreeNode::set_key(parent, 340, Key::empty());
        BtreeNode::set_child(parent, 341, 0);

        fs_data.free_block(right_block).await;
        unsafe { fs_data.remove_inode_cache_entry(right_block) };

        unsafe { fs_data.get_node(left_block).await.0 = true };
        unsafe { fs_data.get_node(parent_block).await.0 = true };

        direction
    }

    //returns the new root
    async fn split_root(node: VirtAddr, block: u32, fs_data: &Rfs) -> u32 {
        let sibling_block = fs_data.allocate_block().await;
        let parent_block = fs_data.allocate_block().await;
        let sibling_node = BtreeNode::new();
        let sibling_virt = sibling_node.virt;
        let parent_node = BtreeNode::new();
        let parent_virt = parent_node.virt;

        unsafe { fs_data.add_node(sibling_block, sibling_node) };
        unsafe { fs_data.add_node(parent_block, parent_node) };

        let separator = BtreeNode::get_key(node, 170);
        BtreeNode::set_key(node, 170, Key::empty());

        for i in 171..341 {
            BtreeNode::set_key(sibling_virt, i - 171, BtreeNode::get_key(node, i));
            BtreeNode::set_child(sibling_virt, i - 170, BtreeNode::get_child(node, i + 1));
            BtreeNode::set_key(node, i, Key::empty());
            BtreeNode::set_child(node, i + 1, 0);
        }
        BtreeNode::set_child(sibling_virt, 171, BtreeNode::get_child(node, 341));

        BtreeNode::set_key(parent_virt, 0, separator);
        BtreeNode::set_child(parent_virt, 0, block);
        BtreeNode::set_child(parent_virt, 1, sibling_block);

        unsafe { fs_data.get_node(block).await.0 = true };

        parent_block
    }

    async fn insert_key_internal(node: VirtAddr, block: u32, parent_block: u32, key: Key, fs_data: &Rfs) -> RebalanceResult {
        let is_leaf = BtreeNode::get_child(node, 0) == 0;
        if is_leaf {
            let is_full = BtreeNode::get_key(node, 340).index != 0;
            if is_full {
                return BtreeNode::insert_full(node, block, parent_block, key, None, fs_data).await;
            } else {
                BtreeNode::insert_non_full(node, block, key, None, fs_data).await;
                return RebalanceResult::None;
            }
        }
        //find first bigger key index
        for i in 0..341 {
            if key.index < BtreeNode::get_key(node, i).index || BtreeNode::get_key(node, i).index == 0 {
                return Box::pin(BtreeNode::insert_into_child(node, block, i, parent_block, key, fs_data)).await;
            }
        }
        Box::pin(BtreeNode::insert_into_child(node, block, 341, parent_block, key, fs_data)).await
    }

    async fn insert_into_child(
        node: VirtAddr,
        block: u32,
        child_index: usize,
        parent_block: u32,
        key: Key,
        fs_data: &Rfs,
    ) -> RebalanceResult {
        let child_node_index = BtreeNode::get_child(node, child_index);
        let rebalance_result = BtreeNode::insert_key_internal(
            unsafe { fs_data.get_node(child_node_index).await.1.virt },
            child_node_index,
            block,
            key,
            fs_data,
        )
        .await;
        match rebalance_result {
            RebalanceResult::None => RebalanceResult::None,
            RebalanceResult::Merge(_) => {
                unreachable!("Nodes should not be merged when inserting keys");
            }
            RebalanceResult::Rotate(_) => {
                //child does everything
                RebalanceResult::None
            }
            RebalanceResult::Split(new_block, new_key) => {
                let self_full = BtreeNode::get_key(node, 340).index != 0;
                if self_full {
                    BtreeNode::insert_full(node, block, parent_block, new_key, Some(new_block), fs_data).await
                } else {
                    BtreeNode::insert_non_full(node, block, new_key, Some(new_block), fs_data).await;
                    RebalanceResult::None
                }
            }
        }
    }

    async fn insert_non_full(node: VirtAddr, block: u32, key: Key, child: Option<u32>, fs_data: &Rfs) {
        let mut ptr: i32 = 339;
        let key_inserted = false;
        while ptr >= 0 && BtreeNode::get_key(node, ptr as usize).index == 0 {
            ptr -= 1;
        }

        println!("inserting into ptr {}", ptr);

        if ptr < 0 {
            BtreeNode::set_key(node, 0, key);
            //is empty root
            return;
        }

        while ptr >= 0 && !key_inserted {
            let current_key = BtreeNode::get_key(node, ptr as usize);
            if current_key.index > key.index {
                BtreeNode::set_key(node, ptr as usize + 1, current_key);
                BtreeNode::set_child(node, ptr as usize + 2, BtreeNode::get_child(node, ptr as usize + 1));
                ptr -= 1;
            } else {
                BtreeNode::set_key(node, ptr as usize + 1, key);
                BtreeNode::set_child(node, ptr as usize + 2, child.unwrap_or(0));
                unsafe { fs_data.get_node(block).await.0 = true };
                return;
            }
        }
        BtreeNode::set_key(node, 0, key);
        BtreeNode::set_child(node, 1, child.unwrap_or(0));
        unsafe { fs_data.get_node(block).await.0 = true };
    }

    ///Child must be on the right of the key
    async fn insert_full(
        node: VirtAddr,
        block: u32,
        parent_block: u32,
        key: Key,
        child: Option<u32>,
        fs_data: &Rfs,
    ) -> RebalanceResult {
        let mut left = true;
        let mut result = BtreeNode::rotate_left_give(block, parent_block, fs_data, child.is_none()).await;
        if !result {
            left = false;
            result = BtreeNode::rotate_right_give(block, parent_block, fs_data, child.is_none()).await;
        }

        if result {
            //-------------------ROTATE SUCCESSFUL-------------------
            //find correct key
            for i in (0..340).rev() {
                let curr_key = BtreeNode::get_key(node, i);
                if curr_key.index == 0 {
                    continue;
                }
                if curr_key.index < key.index {
                    BtreeNode::set_key(node, i + 1, key);
                    BtreeNode::set_child(node, i + 2, child.unwrap_or(0));
                    break;
                }
                BtreeNode::set_key(node, i + 1, BtreeNode::get_key(node, i));
                if child.is_some() {
                    BtreeNode::set_child(node, i + 2, BtreeNode::get_child(node, i + 1));
                }
            }
            unsafe { fs_data.get_node(block).await.0 = true };
            return RebalanceResult::Rotate(if left { RotateDirection::Left } else { RotateDirection::Right });
        }

        //-------------------SPLIT NODE-------------------
        let new_block = fs_data.allocate_block().await;
        let new_node = BtreeNode::new();
        let new_virt = new_node.virt;

        unsafe { fs_data.add_node(new_block, new_node) };
        unsafe { fs_data.get_node(block).await.0 = true };

        //copy half of the elements to the new node, but take care to insert the key when
        //necessary. One node has 341 keys. 170/171 after split
        let mut left_ptr: i32 = 340;
        let mut right_ptr: i32 = 169;
        let mut key_inserted = false;
        while right_ptr >= 0 {
            if key_inserted {
                BtreeNode::set_key(new_virt, right_ptr as usize, BtreeNode::get_key(node, left_ptr as usize));
                BtreeNode::set_child(
                    new_virt,
                    right_ptr as usize + 1,
                    BtreeNode::get_child(node, left_ptr as usize + 1),
                );
                BtreeNode::set_key(node, left_ptr as usize, Key::empty());
                BtreeNode::set_child(node, left_ptr as usize + 1, 0);
                right_ptr -= 1;
                left_ptr -= 1;
                continue;
            }
            let left_key = BtreeNode::get_key(node, left_ptr as usize);
            if left_key.index > key.index {
                BtreeNode::set_key(new_virt, right_ptr as usize, left_key);
                BtreeNode::set_child(
                    new_virt,
                    right_ptr as usize + 1,
                    BtreeNode::get_child(node, left_ptr as usize + 1),
                );
                BtreeNode::set_key(node, left_ptr as usize, Key::empty());
                BtreeNode::set_child(node, left_ptr as usize + 1, 0);
                right_ptr -= 1;
                left_ptr -= 1;
            } else {
                BtreeNode::set_key(new_virt, right_ptr as usize, key);
                BtreeNode::set_child(new_virt, right_ptr as usize + 1, child.unwrap_or(0));
                key_inserted = true;
                right_ptr -= 1;
            }
        }
        while !key_inserted && left_ptr >= 0 {
            let left_key = BtreeNode::get_key(node, left_ptr as usize);
            if left_key.index > key.index {
                BtreeNode::set_key(node, left_ptr as usize + 1, left_key);
                BtreeNode::set_child(node, left_ptr as usize + 2, BtreeNode::get_child(node, left_ptr as usize + 1));
                left_ptr -= 1;
            } else {
                BtreeNode::set_key(node, left_ptr as usize + 1, key);
                BtreeNode::set_child(node, left_ptr as usize + 2, child.unwrap_or(0));
                key_inserted = true;
            }
        }
        if !key_inserted {
            BtreeNode::set_key(node, 0, key);
            BtreeNode::set_child(node, 1, child.unwrap_or(0));
        }

        let separator = BtreeNode::get_key(node, 171);
        BtreeNode::set_key(node, 171, Key::empty());

        unsafe { fs_data.get_node(block).await.0 = true };
        unsafe { fs_data.get_node(new_block).await.0 = true };

        RebalanceResult::Split(new_block, separator)
    }

    async fn rotate_left_take(node: VirtAddr, block: u32, parent_block: u32, fs_data: &Rfs, leaf: bool) -> bool {
        let parent = unsafe { fs_data.get_node(parent_block).await.1.virt };
        let self_index = unsafe { &*(parent.0 as *const BtreeNode) }
            .children
            .iter()
            .position(|&x| x == block)
            .expect("block must be a child of parent");
        if self_index == 0 {
            return false;
        }
        let left_sibling = unsafe { fs_data.get_node(BtreeNode::get_child(parent, self_index - 1)).await };
        let left_key = unsafe { &*(parent.0 as *const BtreeNode) }.keys[self_index - 1];

        let sibling_has_elements = BtreeNode::get_key(left_sibling.1.virt, 170).index != 0;
        let self_has_space = BtreeNode::get_key(node, 340).index == 0;
        if !sibling_has_elements || !self_has_space {
            return false;
        }

        //shift self elements to the right
        let mut ptr: i32 = 339;
        while ptr >= 0 {
            BtreeNode::set_key(node, ptr as usize + 1, BtreeNode::get_key(node, ptr as usize));
            ptr -= 1;
        }
        if !leaf {
            let mut ptr: i32 = 340;
            while ptr >= 0 {
                BtreeNode::set_child(node, ptr as usize + 1, BtreeNode::get_child(node, ptr as usize));
                ptr -= 1;
            }
        }

        //insert the key from the parent
        BtreeNode::set_key(node, 0, left_key);

        let mut last_key_index = 340;
        //find where left sibling's last key is
        for i in (0..340).rev() {
            if BtreeNode::get_key(left_sibling.1.virt, i).index != 0 {
                last_key_index = i;
                break;
            }
        }

        //set parent's key to left sibling's last key
        unsafe { &mut *(parent.0 as *mut BtreeNode) }.keys[self_index - 1] =
            BtreeNode::get_key(left_sibling.1.virt, last_key_index);

        //set self first child to left sibling's last child
        if !leaf {
            BtreeNode::set_child(node, 0, BtreeNode::get_child(left_sibling.1.virt, last_key_index + 1));
        }

        //remove left sibling's last key and child
        BtreeNode::set_key(left_sibling.1.virt, last_key_index, Key::empty());
        if !leaf {
            BtreeNode::set_child(left_sibling.1.virt, last_key_index + 1, 0);
        }

        left_sibling.0 = true;
        unsafe { fs_data.get_node(block).await.0 = true };
        unsafe { fs_data.get_node(parent_block).await.0 = true };

        true
    }

    async fn rotate_right_take(node: VirtAddr, block: u32, parent_block: u32, fs_data: &Rfs, leaf: bool) -> bool {
        let parent = unsafe { fs_data.get_node(parent_block).await.1.virt };
        let self_index = unsafe { &*(parent.0 as *const BtreeNode) }
            .children
            .iter()
            .position(|&x| x == block)
            .expect("block must be a child of parent");
        if unsafe { *(*(parent.0 as *const BtreeNode)).children.get_unchecked(self_index + 1) } == 0 {
            return false;
        }
        let right_sibling = unsafe {
            fs_data
                .get_node((*(parent.0 as *const BtreeNode)).children[self_index + 1])
                .await
        };
        let right_key = BtreeNode::get_key(parent, self_index);

        let sibling_has_elements = BtreeNode::get_key(right_sibling.1.virt, 170).index != 0;
        let self_has_space = BtreeNode::get_key(node, 340).index == 0;
        if !sibling_has_elements || !self_has_space {
            return false;
        }

        let mut last_key_index = 0;
        //find where self's last key is
        for i in (0..340).rev() {
            if BtreeNode::get_key(node, i).index != 0 {
                last_key_index = i;
                break;
            }
        }

        //insert the key from the parent
        BtreeNode::set_key(node, last_key_index + 1, right_key);

        //set self last child to right sibling's first child
        if !leaf {
            BtreeNode::set_child(node, last_key_index + 2, BtreeNode::get_child(right_sibling.1.virt, 0));
        }

        //set parent's key to right sibling's first key
        BtreeNode::set_key(parent, self_index, BtreeNode::get_key(right_sibling.1.virt, 0));

        //shift right sibling's elements to the left
        let mut ptr: i32 = 0;
        while ptr < 340 {
            BtreeNode::set_key(
                right_sibling.1.virt,
                ptr as usize,
                BtreeNode::get_key(right_sibling.1.virt, ptr as usize + 1),
            );
            ptr += 1;
        }
        if !leaf {
            let mut ptr: i32 = 0;
            while ptr < 341 {
                BtreeNode::set_child(
                    right_sibling.1.virt,
                    ptr as usize,
                    BtreeNode::get_child(right_sibling.1.virt, ptr as usize + 1),
                );
                ptr += 1;
            }
        }
        BtreeNode::set_key(right_sibling.1.virt, 340, Key::empty());
        if !leaf {
            BtreeNode::set_child(right_sibling.1.virt, 341, 0);
        }

        right_sibling.0 = true;
        unsafe { fs_data.get_node(block).await.0 = true };
        unsafe { fs_data.get_node(parent_block).await.0 = true };

        true
    }

    async fn rotate_left_give(block: u32, parent_block: u32, fs_data: &Rfs, leaf: bool) -> bool {
        let parent = unsafe { fs_data.get_node(parent_block).await.1.virt };
        let self_index = unsafe { &*(parent.0 as *const BtreeNode) }
            .children
            .iter()
            .position(|&x| x == block)
            .expect("block must be a child of parent");
        if self_index == 0 {
            return false;
        }
        let left_sibling_block = unsafe { &*(parent.0 as *const BtreeNode) }.children[self_index - 1];
        let left_sibling = unsafe { fs_data.get_node(left_sibling_block).await.1.virt };
        BtreeNode::rotate_right_take(left_sibling, left_sibling_block, parent_block, fs_data, leaf).await
    }

    async fn rotate_right_give(block: u32, parent_block: u32, fs_data: &Rfs, leaf: bool) -> bool {
        let parent = unsafe { fs_data.get_node(parent_block).await.1.virt };
        let self_index = unsafe { &*(parent.0 as *const BtreeNode) }
            .children
            .iter()
            .position(|&x| x == block)
            .expect("block must be a child of parent");
        if self_index == 341 || unsafe { *(*(parent.0 as *const BtreeNode)).children.get_unchecked(self_index + 1) } == 0 {
            return false;
        }
        let right_sibling_block = unsafe { &*(parent.0 as *const BtreeNode) }.children[self_index + 1];
        let right_sibling = unsafe { fs_data.get_node(right_sibling_block).await.1.virt };
        BtreeNode::rotate_right_take(right_sibling, right_sibling_block, parent_block, fs_data, leaf).await
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Key {
    pub index: u32,
    pub inode_block: u32,
}

impl Key {
    fn empty() -> Self {
        Self {
            index: 0,
            inode_block: 0,
        }
    }
}

//rotations are done by children, not recorded here
enum RebalanceResult {
    ///always merge right into left so left doesn't have to be rearranged. Children should merge
    Merge(MergeDirection),
    ///Rotate left or right. Child already does this, but parent should be saved
    Rotate(RotateDirection),
    ///Always split left into right. u32 is the address of the right block, tuple is the key + value address
    Split(u32, Key),
    None,
}

enum MergeDirection {
    RightToCurrent,
    CurrentToLeft,
}

enum RotateDirection {
    Left,
    Right,
}
