use std::boxed::Box;

use crate::drivers::filesystem::rfs2::{
    BlockPtr, InodeIndex, Rfs2,
    btree::{BTREE_KEY_CNT, BTreeNode, FillState, Key, LevelState},
};

#[derive(PartialEq, Eq)]
enum RemoveResult {
    Nothing,
    Updated,
    TooShort,
}

impl BTreeNode {
    pub async fn remove_key_root(inode_index: InodeIndex, rfs: &Rfs2) -> Option<BlockPtr> {
        let mut superblock = rfs.get_superblock().await;
        if superblock.inode_tree_root_ptr == 0 {
            panic!("illegal root pointer");
        }
        let mut root_block = rfs.get_inode_tree_block(superblock.inode_tree_root_ptr).await;
        let root_node = root_block.get_as_mut::<BTreeNode>();
        let result = Box::pin(root_node.remove_key(inode_index, rfs)).await;
        let Some((block, rmv_res)) = result else {
            root_block.forget_mem_binding();
            return None;
        };
        if root_node.key_indexes[0] == 0 {
            superblock.inode_tree_root_ptr = root_node.children[0];
            rfs.update_superblock(superblock).await;
            rfs.delete_inode_tree_block(root_block).await;
        } else if rmv_res == RemoveResult::Updated || rmv_res == RemoveResult::TooShort {
            rfs.update_inode_tree_block(root_block);
        } else {
            root_block.forget_mem_binding();
        }
        Some(block)
    }

    fn remove_leaf_non_empty(&mut self, index: InodeIndex) -> Option<(BlockPtr, RemoveResult)> {
        for i in 0..BTREE_KEY_CNT {
            if self.key_indexes[i] == index {
                let block = self.key_ptrs[i];
                for j in i..BTREE_KEY_CNT - 1 {
                    self.key_indexes[j] = self.key_indexes[j + 1];
                    self.key_ptrs[j] = self.key_ptrs[j + 1];
                }
                self.key_indexes[BTREE_KEY_CNT - 1] = 0;
                self.key_ptrs[BTREE_KEY_CNT - 1] = 0;

                if self.key_indexes[BTREE_KEY_CNT / 2 - 1] == 0 {
                    return Some((block, RemoveResult::TooShort));
                }

                return Some((block, RemoveResult::Updated));
            } else if self.key_indexes[i] == 0 {
                break;
            }
        }
        None
    }

    async fn remove_key(&mut self, index: InodeIndex, rfs: &Rfs2) -> Option<(BlockPtr, RemoveResult)> {
        let state = self.get_state();

        if state.0 == FillState::Empty {
            return None;
        }
        if state.1 == LevelState::Leaf {
            return self.remove_leaf_non_empty(index);
        }

        for i in 0..BTREE_KEY_CNT {
            if self.key_indexes[i] == index {
                let block = self.key_ptrs[i];
                for j in i..BTREE_KEY_CNT - 1 {
                    self.key_indexes[j] = self.key_indexes[j + 1];
                    self.key_ptrs[j] = self.key_ptrs[j + 1];
                }

                let child = self.children[i];
                let mut child_block = rfs.get_inode_tree_block(child).await;
                let child_node = child_block.get_as_mut::<BTreeNode>();
                let child_result = child_node.take_largest_key(rfs).await;
                let Some((key, remove_result)) = child_result else {
                    panic!("child is not empty but could not take largest key");
                };

                self.key_indexes[BTREE_KEY_CNT - 1] = key.index;
                self.key_ptrs[BTREE_KEY_CNT - 1] = key.ptr;

                if remove_result == RemoveResult::TooShort {
                    let handling_result = self.handle_short_child(child_node, i, rfs).await;
                    if handling_result == RemoveResult::TooShort {
                        rfs.update_inode_tree_block(child_block);
                        return Some((block, RemoveResult::TooShort));
                    }
                } else if remove_result == RemoveResult::Updated {
                    rfs.update_inode_tree_block(child_block);
                } else {
                    child_block.forget_mem_binding();
                }
                return Some((block, RemoveResult::Updated));
            } else if self.key_indexes[i] > index {
                let child_index = i;
                let mut child_block = rfs.get_inode_tree_block(self.children[child_index]).await;
                let child_node = child_block.get_as_mut::<BTreeNode>();
                let result = Box::pin(child_node.remove_key(index, rfs)).await;
                let Some((block, remove_result)) = result else {
                    child_block.forget_mem_binding();
                    return None;
                };

                if remove_result == RemoveResult::TooShort {
                    let child_result = self.handle_short_child(child_node, child_index, rfs).await;
                    rfs.update_inode_tree_block(child_block);

                    return Some((block, child_result));
                } else if remove_result == RemoveResult::Updated {
                    rfs.update_inode_tree_block(child_block);
                } else {
                    child_block.forget_mem_binding();
                }
                return Some((block, RemoveResult::Nothing));
            } else if self.key_indexes[i] == 0 {
                break;
            }
        }
        None
    }

    //return Self remove result
    async fn handle_short_child(&mut self, child: &mut Self, child_index: usize, rfs: &Rfs2) -> RemoveResult {
        if child_index > 0 {
            //try rotate and merge with left
            let mut left_child_block = rfs.get_inode_tree_block(self.children[child_index - 1]).await;
            let left_child = left_child_block.get_as_mut::<BTreeNode>();
            if Self::try_rotate_right(self, child_index - 1, left_child, child) {
                rfs.update_inode_tree_block(left_child_block);
                return RemoveResult::Updated;
            }
            if Self::try_merge_ltr(self, child_index - 1, left_child, child) {
                rfs.delete_inode_tree_block(left_child_block).await;
                if self.get_state().0 == FillState::TooShort {
                    return RemoveResult::TooShort;
                } else {
                    return RemoveResult::Updated;
                }
            }
            panic!("left child exists but could neither rotate nor merge");
        }

        let mut right_child_block = rfs.get_inode_tree_block(self.children[child_index + 1]).await;
        let right_child = right_child_block.get_as_mut::<BTreeNode>();
        if Self::try_rotate_left(self, child_index, child, right_child) {
            rfs.update_inode_tree_block(right_child_block);
            return RemoveResult::Updated;
        }
        if Self::try_merge_rtl(self, child_index, child, right_child) {
            rfs.delete_inode_tree_block(right_child_block).await;
            if self.get_state().0 == FillState::TooShort {
                return RemoveResult::TooShort;
            } else {
                return RemoveResult::Updated;
            }
        }

        panic!("right child exists but could neihter rotate nor merge");
    }

    //returns None if it's empty
    async fn take_largest_key(&mut self, rfs: &Rfs2) -> Option<(Key, RemoveResult)> {
        let state = self.get_state();

        if state.0 == FillState::Empty {
            return None;
        }

        if state.1 == LevelState::Leaf {
            self.take_leaf_key(state.0 == FillState::LowerLimit, true)
        } else {
            self.take_non_leaf_key(true, rfs).await
        }
    }

    async fn take_smallest_key(&mut self, rfs: &Rfs2) -> Option<(Key, RemoveResult)> {
        let state = self.get_state();

        if state.0 == FillState::Empty {
            return None;
        }

        if state.1 == LevelState::Leaf {
            self.take_leaf_key(state.0 == FillState::LowerLimit, false)
        } else {
            self.take_non_leaf_key(false, rfs).await
        }
    }

    fn take_leaf_key(&mut self, is_lower_limit: bool, largest: bool) -> Option<(Key, RemoveResult)> {
        let key_index = if largest {
            self.key_indexes.iter().position(|k| *k == 0).unwrap_or(BTREE_KEY_CNT) - 1
        } else {
            0
        };

        let key = Key {
            index: self.key_indexes[key_index],
            ptr: self.key_ptrs[key_index],
        };
        if largest {
            self.key_indexes[key_index] = 0;
            self.key_ptrs[key_index] = 0;
        } else {
            for i in 0..BTREE_KEY_CNT - 1 {
                if self.key_indexes[i + 1] == 0 {
                    self.key_indexes[i] = 0;
                    self.key_ptrs[i] = 0;
                    break;
                }
                self.key_indexes[i] = self.key_indexes[i + 1];
                self.key_ptrs[i] = self.key_ptrs[i + 1];
            }
        }
        if is_lower_limit {
            return Some((key, RemoveResult::TooShort));
        }
        Some((key, RemoveResult::Updated))
    }

    async fn take_non_leaf_key(&mut self, largest: bool, rfs: &Rfs2) -> Option<(Key, RemoveResult)> {
        let key_index = if largest {
            self.key_indexes.iter().position(|k| *k == 0).unwrap_or(BTREE_KEY_CNT) - 1
        } else {
            0
        };
        let child_index = if largest { key_index + 1 } else { key_index };
        let mut child_block = rfs.get_inode_tree_block(self.children[child_index]).await;
        let child_node = child_block.get_as_mut::<BTreeNode>();

        let result = if largest {
            Box::pin(child_node.take_largest_key(rfs)).await
        } else {
            Box::pin(child_node.take_smallest_key(rfs)).await
        };

        let Some((key, remove_result)) = result else {
            panic!("child is not empty but could not take largest key");
        };
        if remove_result == RemoveResult::TooShort {
            let child_result = self.handle_short_child(child_node, child_index, rfs).await;
            rfs.update_inode_tree_block(child_block);

            return Some((key, child_result));
        } else if remove_result == RemoveResult::Updated {
            rfs.update_inode_tree_block(child_block);
        } else {
            child_block.forget_mem_binding();
        }
        Some((key, RemoveResult::Nothing))
    }

    fn try_merge_rtl(parent: &mut Self, key_index: usize, left: &mut Self, right: &mut Self) -> bool {
        let left_key_cnt = left.get_num_keys();
        let right_key_cnt = right.get_num_keys();
        if left_key_cnt + right_key_cnt + 1 > BTREE_KEY_CNT {
            return false;
        }

        left.key_indexes[left_key_cnt] = parent.key_indexes[key_index];
        left.key_ptrs[left_key_cnt] = parent.key_ptrs[key_index];

        for i in 0..right_key_cnt {
            left.children[i + left_key_cnt + 1] = right.children[i];
            left.key_indexes[i + left_key_cnt + 1] = right.key_indexes[i];
            left.key_ptrs[i + left_key_cnt + 1] = right.key_ptrs[i];
        }
        left.children[left_key_cnt + 1 + right_key_cnt] = right.children[right_key_cnt];

        for i in key_index..(BTREE_KEY_CNT - 1) {
            parent.key_ptrs[i] = parent.key_ptrs[i + 1];
            parent.key_indexes[i] = parent.key_indexes[i + 1];
            parent.children[i + 1] = parent.children[i + 2];
        }
        parent.key_ptrs[BTREE_KEY_CNT - 1] = 0;
        parent.key_indexes[BTREE_KEY_CNT - 1] = 0;
        parent.children[BTREE_KEY_CNT] = 0;
        true
    }

    ///Left block becomes invalid
    fn try_merge_ltr(parent: &mut Self, key_index: usize, left: &mut Self, right: &mut Self) -> bool {
        let right_ptr = parent.children[key_index + 1];
        if !Self::try_merge_rtl(parent, key_index, left, right) {
            return false;
        }
        right.children = left.children;
        right.key_indexes = left.key_indexes;
        right.key_ptrs = left.key_ptrs;
        parent.children[key_index] = right_ptr;
        true
    }
}
