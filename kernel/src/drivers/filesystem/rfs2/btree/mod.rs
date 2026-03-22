use crate::drivers::filesystem::rfs2::BlockPtr;

use super::InodeIndex;

mod find;
mod insert;
mod remove;

const MAX_NODE_SIZE: usize = 4096;
const PTR_SIZE: usize = core::mem::size_of::<BlockPtr>();
const INDEX_SIZE: usize = core::mem::size_of::<InodeIndex>();
const IDEAL_BTREE_KEY_CNT: usize = (MAX_NODE_SIZE - PTR_SIZE) / (INDEX_SIZE + 2 * PTR_SIZE); //204
const BTREE_KEY_CNT: usize = IDEAL_BTREE_KEY_CNT & !1; //make it even

#[derive(PartialEq, Eq)]
enum FillState {
    Empty,
    TooShort,
    LowerLimit,
    PartFilled,
    Full,
}

impl FillState {
    fn can_take(&self) -> bool {
        matches!(self, FillState::PartFilled | FillState::Full)
    }

    fn can_give(&self) -> bool {
        matches!(
            self,
            FillState::LowerLimit | FillState::TooShort | FillState::PartFilled | FillState::Empty
        )
    }
}

#[derive(PartialEq, Eq)]
enum LevelState {
    Leaf,
    NonLeaf,
}

#[repr(C)]
pub(super) struct BTreeNode {
    children: [BlockPtr; BTREE_KEY_CNT + 1],
    key_indexes: [InodeIndex; BTREE_KEY_CNT],
    key_ptrs: [BlockPtr; BTREE_KEY_CNT],
}

#[derive(PartialEq, Eq)]
#[repr(C)]
struct Key {
    index: InodeIndex,
    ptr: BlockPtr,
}

impl BTreeNode {
    pub fn initialize_root(&mut self, root_inode_index: InodeIndex, root_inode_ptr: BlockPtr) {
        for i in 0..BTREE_KEY_CNT {
            self.children[i] = 0;
            self.key_indexes[i] = 0;
            self.key_ptrs[i] = 0;
        }
        self.children[BTREE_KEY_CNT] = 0;

        self.key_indexes[0] = root_inode_index;
        self.key_ptrs[0] = root_inode_ptr;
    }

    fn get_state(&self) -> (FillState, LevelState) {
        let empty = self.key_indexes[0] == 0;
        let full = self.key_indexes[0] != 0;
        let lower_bound = self.key_indexes[BTREE_KEY_CNT / 2] == 0;
        let too_short = self.key_indexes[BTREE_KEY_CNT / 2 - 1] == 0;

        let leaf = self.children[0] == 0;

        let fill_state = match (empty, too_short, full, lower_bound) {
            (true, _, _, _) => FillState::Empty,
            (false, true, _, _) => FillState::TooShort,
            (false, false, true, _) => FillState::Full,
            (false, false, false, true) => FillState::LowerLimit,
            (false, false, false, false) => FillState::PartFilled,
        };
        let level_state = match leaf {
            true => LevelState::Leaf,
            false => LevelState::NonLeaf,
        };
        (fill_state, level_state)
    }

    fn get_num_keys(&self) -> usize {
        for i in 0..BTREE_KEY_CNT {
            if self.key_indexes[i] == 0 {
                return i;
            }
        }
        BTREE_KEY_CNT
    }

    fn try_rotate_right(parent: &mut Self, key_index: usize, left: &mut Self, right: &mut Self) -> bool {
        if left.key_indexes[0] == 0 {
            return false;
        }
        if right.key_indexes[BTREE_KEY_CNT - 1] != 0 {
            return false;
        }

        let left_last_valid_index = (0..BTREE_KEY_CNT)
            .find(|i| left.key_indexes[*i] == 0)
            .unwrap_or(BTREE_KEY_CNT)
            - 1;

        for i in (0..BTREE_KEY_CNT - 1).rev() {
            if right.key_indexes[i] == 0 {
                continue;
            }
            right.key_indexes[i + 1] = right.key_indexes[i];
            right.key_ptrs[i + 1] = right.key_ptrs[i];
            right.children[i + 2] = right.children[i + 1];
        }
        right.children[1] = right.children[0];

        right.key_indexes[0] = parent.key_indexes[key_index];
        right.key_ptrs[0] = parent.key_ptrs[key_index];
        right.children[0] = left.children[left_last_valid_index + 1];

        parent.key_indexes[key_index] = left.key_indexes[left_last_valid_index];
        parent.key_ptrs[key_index] = left.key_ptrs[left_last_valid_index];

        left.key_indexes[left_last_valid_index] = 0;
        left.key_ptrs[left_last_valid_index] = 0;
        left.children[left_last_valid_index + 1] = 0;
        true
    }

    fn try_rotate_left(parent: &mut Self, key_index: usize, left: &mut Self, right: &mut Self) -> bool {
        if right.key_indexes[0] == 0 {
            return false;
        }
        if left.key_indexes[BTREE_KEY_CNT - 1] != 0 {
            return false;
        }

        let left_first_empty_index = (0..BTREE_KEY_CNT).find(|i| left.key_indexes[*i] == 0).expect("is not full");

        left.key_indexes[left_first_empty_index] = parent.key_indexes[key_index];
        left.key_ptrs[left_first_empty_index] = parent.key_ptrs[key_index];
        left.children[left_first_empty_index + 1] = right.children[0];

        parent.key_indexes[key_index] = right.key_indexes[0];
        parent.key_ptrs[key_index] = right.key_ptrs[0];

        for i in 0..BTREE_KEY_CNT - 1 {
            if right.key_indexes[i] == 0 {
                break;
            }
            right.key_indexes[i] = right.key_indexes[i + 1];
            right.key_ptrs[i] = right.key_ptrs[i + 1];
            right.children[i] = right.children[i + 1];
        }

        right.key_indexes[BTREE_KEY_CNT - 1] = 0;
        right.key_ptrs[BTREE_KEY_CNT - 1] = 0;
        right.children[BTREE_KEY_CNT - 1] = right.children[BTREE_KEY_CNT];
        right.children[BTREE_KEY_CNT] = 0;

        true
    }
}
