# RFS 2 documentation

RFS 2 is the second iteration of the filesystem made for meowOS

## Structure: 
The disk is divided into blocks, each 4kB. Most accesses use blocks, not sectors
The disk is also divided into 'groups'. Those are regions of 4096 * 8 blocks
All pointers to disk are 64 bits in size, and are indexes of blocks (not sectors) on the disk
Inode indexes are 32 bits in size

### Used blocks bitmask:
The first block in each group contains a bitmask of used blocks in that group. 
Each bit corresponds to a block, and is set to 1 if the block is used, and 0 if it is free

### Superblock:
The superblock structure contains the following fields:
```rs
#[repr(C)]
struct SuperBlock {
    inode_mask_ptr: u64,
    root_inode_index: u32,
    inode_tree_root_ptr: u64,
    fs_size: u64,
    crc32: u32,
}
```
inode_mask_ptr points to the first element in the inode mask list.
root_inode_index is the index of the root inode, which depends on the operating system using it, but should probably be 2.
inode_tree_root_ptr: points to the root of the inode tree
fs_size: the size of the filesystem in blocks
crc32: the crc32 of the superblock, with this field set to 0

It occurs as the second block of every 64th group. If checksum is incorrect and there are multiple superbocks, load a valid superblock.
Checksum is xor of 32 bit words (potentially extended to full 32 bits with 0 bits)

### File structure: 
The file starts at some block. The first sector of that block contains the inode information. The other 7 sectors either...
 - contain a file (file size is less than or equal to 3584B)
 - contain pointers to other blocks
Each pointer is 64bit wide (8B). It points to another block that either contains the file content or more pointers
There are a maximum of 4 pointer levels (first is inside the initial file block), allowing for a maximum of 314TB file

### Inode tree:
To efficiently find a file using its inode index, the (inode_index -> file_block_ptr) map is implemented as a BTree map
Each BTree node has the following structure
```rs
#[repr(C)]
struct BTreeNode {
    children: [u64; BTREE_KEY_CNT + 1],
    keys: [Key; BTREE_KEY_CNT],
}

#[repr(C)]
struct Key {
    index: u32,
    ptr: u64,
}

const BTREE_KEY_CNT: u64 = (4096 - 8) / (8 + 12); //204

```
The children array points to blocks containing another BTreeNode
Each key contains an inode index and a pointer to its coresponding file block
Keys in a node are always left aligned. Apart from the root node, 

### Formatted partition
Each group starts with a block bitmask (at least the first bit is set)
Every 64 groups, the second block is a superblock (with a valid checksum)
The third (index 2) block in the first (index 0) group is the root of the inode tree
The fourth (index 3) block in the first (index 0) group is the inode bitmask
The fifth (index 4) block in the first (index 0) group is the root inode block
