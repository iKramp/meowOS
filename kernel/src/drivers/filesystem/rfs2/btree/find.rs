use std::boxed::Box;

use crate::drivers::filesystem::rfs2::{
    BlockPtr, InodeIndex, Rfs2,
    btree::{BTREE_KEY_CNT, BTreeNode},
};

impl BTreeNode {
    pub async fn find_inode_root(inode_index: InodeIndex, rfs: &Rfs2) -> Option<BlockPtr> {
        let superblock = rfs.get_superblock().await;
        if superblock.inode_tree_root_ptr == 0 {
            panic!("Inode tree root pointer is 0");
        }
        let root_block = rfs.get_disk_block(superblock.inode_tree_root_ptr).await;
        let root_node = root_block.get_as::<BTreeNode>();
        let res = root_node.find_inode(inode_index, rfs).await;
        root_block.forget_mem_binding();
        res
    }

    async fn find_inode(&self, inode_index: InodeIndex, rfs: &Rfs2) -> Option<BlockPtr> {
        for i in 0..BTREE_KEY_CNT {
            if self.key_indexes[i] == 0 {
                break;
            }
            if self.key_indexes[i] < inode_index {
                let child_ptr = self.children[i];
                if child_ptr == 0 {
                    return None;
                }
                let child_block = rfs.get_disk_block(child_ptr).await;
                let child_node = child_block.get_as::<BTreeNode>();
                let res = Box::pin(child_node.find_inode(inode_index, rfs)).await;
                child_block.forget_mem_binding();
                return res;
            }
            if self.key_indexes[i] == inode_index {
                return Some(self.key_ptrs[i]);
            }
        }
        if self.children[BTREE_KEY_CNT] != 0 {
            return Some(self.children[BTREE_KEY_CNT]);
        }
        None
    }
}
