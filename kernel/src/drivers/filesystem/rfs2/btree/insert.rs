use std::boxed::Box;

use crate::drivers::filesystem::rfs2::{
    BlockPtr, InodeIndex, Rfs2, WorkingBlock,
    btree::{BTREE_KEY_CNT, BTreeNode, FillState, Key, LevelState},
};

#[derive(PartialEq, Eq)]
enum InsertResult {
    Nothing,
    Updated,
    TooBig(Key, BlockPtr), //blockPtr to the right of the key
}

//first rebalance, then insert

impl BTreeNode {
    pub async fn insert_inode_root(index: InodeIndex, block: BlockPtr, rfs: &Rfs2) {
        let mut superblock = rfs.get_superblock().await;
        if superblock.inode_tree_root_ptr == 0 {
            panic!("illegal root pointer");
        }
        let mut root_block = rfs.get_disk_block(superblock.inode_tree_root_ptr).await;
        let root_node = root_block.get_as_mut::<BTreeNode>();
        let res = root_node.insert_inode(index, block, rfs).await;
        let InsertResult::TooBig(key, child) = res else {
            if res != InsertResult::Updated {
                root_block.changed = false;
            }
            root_block.write_and_dealloc(rfs).await;
            return;
        };
        let mut new_block = WorkingBlock::get_disk_block(rfs).await;
        let new_block_disk_block = new_block.disk_block.expect("wawa");
        let new_node = new_block.get_as_mut::<BTreeNode>();

        for i in 0..BTREE_KEY_CNT {
            new_node.key_indexes[i] = 0;
            new_node.key_ptrs[i] = 0;
            new_node.children[i] = 0;
        }
        new_node.children[BTREE_KEY_CNT] = 0;

        new_node.children[0] = superblock.inode_tree_root_ptr;
        superblock.inode_tree_root_ptr = new_block_disk_block;
        rfs.update_superblock(superblock).await;

        Self::split(new_node, 0, root_node, rfs).await;
        if new_node.key_indexes[0] > key.index {
            root_node.insert_key_child(key, child);
        } else {
            let mut right_child = rfs.get_disk_block(new_node.children[1]).await;
            let right_node = right_child.get_as_mut::<BTreeNode>();
            right_node.insert_key_child(key, child);
            right_child.write_and_dealloc(rfs).await;
        }
        root_block.write_and_dealloc(rfs).await;
        new_block.write_and_dealloc(rfs).await;
    }

    async fn insert_inode(&mut self, index: InodeIndex, block: BlockPtr, rfs: &Rfs2) -> InsertResult {
        let (fill_state, level_state) = self.get_state();

        for i in 0..BTREE_KEY_CNT {
            if self.key_indexes[i] == index {
                self.key_ptrs[i] = block;
                return InsertResult::Updated;
            } else if self.key_indexes[i] == 0 {
                break;
            }
        }

        if level_state == LevelState::Leaf {
            if fill_state == FillState::Full {
                return InsertResult::TooBig(Key { index, ptr: block }, 0);
            } else {
                self.insert_leaf_non_full(index, block);
                return InsertResult::Updated;
            }
        }

        //find child
        let position = self.key_indexes.iter().position(|k| *k > index).unwrap_or(BTREE_KEY_CNT);
        let child_block_ptr = self.children[position];
        let mut child_block = rfs.get_disk_block(child_block_ptr).await;
        let child_node = child_block.get_as_mut::<BTreeNode>();
        let insert_res = Box::pin(child_node.insert_inode(index, block, rfs)).await;
        let InsertResult::TooBig(insert_fail_key, insert_fail_child) = insert_res else {
            if insert_res != InsertResult::Updated {
                child_block.changed = false;
            }
            child_block.write_and_dealloc(rfs).await;
            return InsertResult::Nothing;
        };

        if position > 0 {
            let mut left_sibling_block = rfs.get_disk_block(self.children[position - 1]).await;
            let left_sibling_node = left_sibling_block.get_as_mut::<BTreeNode>();
            if BTreeNode::try_rotate_right(self, position - 1, left_sibling_node, child_node) {
                if self.key_indexes[position - 1] > index {
                    left_sibling_node.insert_key_child(insert_fail_key, insert_fail_child);
                } else {
                    child_node.insert_key_child(insert_fail_key, insert_fail_child);
                }

                left_sibling_block.write_and_dealloc(rfs).await;
                child_block.write_and_dealloc(rfs).await;
                return InsertResult::Updated;
            }
        } else {
            let mut right_sibling_block = rfs.get_disk_block(self.children[position + 1]).await;
            let right_sibling_node = right_sibling_block.get_as_mut::<BTreeNode>();
            if BTreeNode::try_rotate_right(self, position, right_sibling_node, child_node) {
                if self.key_indexes[position] < index {
                    right_sibling_node.insert_key_child(insert_fail_key, insert_fail_child);
                } else {
                    child_node.insert_key_child(insert_fail_key, insert_fail_child);
                }

                right_sibling_block.write_and_dealloc(rfs).await;
                child_block.write_and_dealloc(rfs).await;
                return InsertResult::Updated;
            }
        }

        let res = Self::split(self, position, child_node, rfs).await;
        match res {
            InsertResult::Nothing => {
                panic!("split should never return nothing");
            }
            InsertResult::TooBig(new_fail_key, new_fail_child) => {
                if new_fail_key.index > insert_fail_key.index {
                    child_node.insert_key_child(insert_fail_key, insert_fail_child);
                } else {
                    let mut to_insert_into = rfs.get_disk_block(new_fail_child).await;
                    let to_insert_into_node = to_insert_into.get_as_mut::<BTreeNode>();
                    to_insert_into_node.insert_key_child(insert_fail_key, insert_fail_child);
                    to_insert_into.write_and_dealloc(rfs).await;
                }
                child_block.write_and_dealloc(rfs).await;
                InsertResult::TooBig(new_fail_key, new_fail_child)
            }
            InsertResult::Updated => {
                let key_index = position;
                if self.key_indexes[key_index] > insert_fail_key.index {
                    child_node.insert_key_child(insert_fail_key, insert_fail_child);
                } else {
                    let mut right_sibling_block = rfs.get_disk_block(self.children[position + 1]).await;
                    let right_sibling_node = right_sibling_block.get_as_mut::<BTreeNode>();
                    right_sibling_node.insert_key_child(insert_fail_key, insert_fail_child);
                    right_sibling_block.write_and_dealloc(rfs).await;
                }
                child_block.write_and_dealloc(rfs).await;
                InsertResult::Updated
            }
        }
    }

    //only after checking that the node is not full
    fn insert_key_child(&mut self, key: Key, block: BlockPtr) {
        let position = self
            .key_indexes
            .iter()
            .position(|k| *k > key.index || *k == 0)
            .expect("is not full");
        for i in (position..BTREE_KEY_CNT - 1).rev() {
            self.key_indexes[i + 1] = self.key_indexes[i];
            self.key_ptrs[i + 1] = self.key_ptrs[i];
            self.children[i + 2] = self.children[i + 1];
        }
        self.key_indexes[position] = key.index;
        self.key_ptrs[position] = key.ptr;
        self.children[position + 1] = block;
    }

    pub fn insert_leaf_non_full(&mut self, index: InodeIndex, block: BlockPtr) {
        for i in (0..BTREE_KEY_CNT - 1).rev() {
            if self.key_indexes[i] == 0 {
                continue;
            }

            if self.key_indexes[i] > index {
                self.key_indexes[i + 1] = self.key_indexes[i];
            } else {
                self.key_indexes[i + 1] = index;
                self.key_ptrs[i + 1] = block;
                return;
            }
        }
        self.key_indexes[0] = index;
        self.key_ptrs[0] = block;
    }

    async fn split(parent: &mut Self, child_index: usize, child: &mut Self, rfs: &Rfs2) -> InsertResult {
        let mut new_node_block = WorkingBlock::get_disk_block(rfs).await;
        let new_node_disk_block = new_node_block.disk_block.expect("wawa");
        let new_node = new_node_block.get_as_mut::<BTreeNode>();
        for i in 0..BTREE_KEY_CNT {
            new_node.key_indexes[i] = 0;
            new_node.key_ptrs[i] = 0;
            new_node.children[i] = 0;
        }
        new_node.children[BTREE_KEY_CNT] = 0;

        let split_key_index = child.key_indexes[BTREE_KEY_CNT / 2];
        let split_key_ptr = child.key_ptrs[BTREE_KEY_CNT / 2];

        for i in (BTREE_KEY_CNT / 2 + 1)..BTREE_KEY_CNT {
            let new_index = i - BTREE_KEY_CNT / 2 - 1;
            new_node.key_indexes[new_index] = child.key_indexes[i];
            new_node.key_ptrs[new_index] = child.key_ptrs[i];
            new_node.children[new_index] = child.children[i];
        }
        new_node.children[BTREE_KEY_CNT - BTREE_KEY_CNT / 2] = child.children[BTREE_KEY_CNT];

        new_node_block.write_and_dealloc(rfs).await;

        if parent.key_indexes[BTREE_KEY_CNT - 1] != 0 {
            return InsertResult::TooBig(
                Key {
                    index: split_key_index,
                    ptr: split_key_ptr,
                },
                new_node_disk_block,
            );
        }

        //has space at the end
        for i in ((child_index + 1)..BTREE_KEY_CNT).rev() {
            parent.key_indexes[i] = parent.key_indexes[i - 1];
            parent.key_ptrs[i] = parent.key_ptrs[i - 1];
            parent.children[i + 1] = parent.children[i];
        }

        parent.key_indexes[child_index] = split_key_index;
        parent.key_ptrs[child_index] = split_key_ptr;
        parent.children[child_index + 1] = new_node_disk_block;

        InsertResult::Updated
    }
}
