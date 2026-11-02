//!Module for a static filesystem. It is read only. All files are baked into the kernel binary so
//!there is no need for constant disk reads

use std::{
    alloc::{boxed::Box, string::ToString, vec::Vec},
    error::KernelError,
    kerror,
    sync::{arc::Arc, once_lock::OnceLock},
    vec,
};

use crate::{
    drivers::block_device::disk::DirEntry,
    memory::addresses::{PhysAddr, VirtAddr},
    vfs::{
        DeviceDetails, DeviceId, FileReadResult, FileSystem, Inode, InodeExtraData, InodeIndex, ROOT_INODE_INDEX,
        adapters::VfsAdapterTrait, file::FileHandleExtraData,
    },
};

enum StaticFsNodeType {
    File(&'static [u8]),
    Directory(&'static [StaticFsNode]),
}

struct StaticFsNode {
    name: &'static str,
    inode_index: InodeIndex,
    node_type: StaticFsNodeType,
}

#[derive(Debug)]
pub struct StaticFsAdapter {
    device_id: DeviceId,
    device_details: DeviceDetails,
}

use std::slice;

struct StaticFsNodeIter {
    stack: Vec<slice::Iter<'static, StaticFsNode>>,
}

impl StaticFsNodeIter {
    /// Creates a new iterator that will traverse the given root nodes in pre-order DFS.
    fn new(root: &'static [StaticFsNode]) -> Self {
        Self {
            stack: vec![root.iter()],
        }
    }
}

impl Iterator for StaticFsNodeIter {
    type Item = &'static StaticFsNode;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(iter) = self.stack.last_mut() {
            if let Some(node) = iter.next() {
                if let StaticFsNodeType::Directory(children) = &node.node_type {
                    // Push children to stack for depth-first traversal
                    self.stack.push(children.iter());
                }
                return Some(node);
            } else {
                // Current branch exhausted, backtrack
                self.stack.pop();
            }
        }
        None
    }
}

static STATIC_FS_ADAPTER: OnceLock<Arc<dyn FileSystem + Send>> = OnceLock::new();

static DOOM_WAD: &[u8] = include_bytes!("../../../../fat_disk/doom/devdata/doom1.wad");

static DOOM_FS_TREE: &[StaticFsNode] = &[StaticFsNode {
    name: "doom",
    inode_index: ROOT_INODE_INDEX,
    node_type: StaticFsNodeType::Directory(&[StaticFsNode {
        name: "devdata",
        inode_index: ROOT_INODE_INDEX + 1,
        node_type: StaticFsNodeType::Directory(&[StaticFsNode {
            name: "doom1.wad",
            inode_index: ROOT_INODE_INDEX + 2,
            node_type: StaticFsNodeType::File(DOOM_WAD),
        }]),
    }]),
}];

impl StaticFsAdapter {
    pub fn get() -> Arc<dyn FileSystem + Send> {
        STATIC_FS_ADAPTER
            .get_or_init(|| {
                let device_details =
                    crate::vfs::VFS_ADAPTER_DEVICE.allocate_device(&mut crate::vfs::lock_w_info!(crate::vfs::VFS));
                Arc::new(Self {
                    device_id: device_details.0,
                    device_details: device_details.1,
                })
            })
            .clone()
    }

    fn get_inode(&self, index: InodeIndex) -> Inode {
        let mut inode = Inode {
            index,
            device: self.device_id,
            type_mode: crate::vfs::InodeTypeAndPerms::new_file(0o777),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: 0,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
            internal_synchronization: true,
            requires_aligned_position: false,
            extra_data: Box::new(crate::vfs::EmptyExtraInodeData),
        };

        //find the node in the static fs tree
        let mut found_node = None;
        let iter = StaticFsNodeIter::new(DOOM_FS_TREE);
        for node in iter {
            if node.inode_index == index {
                found_node = Some(node);
                break;
            }
        }

        if let Some(node) = found_node {
            match &node.node_type {
                StaticFsNodeType::File(data) => {
                    inode.size = data.len() as u64;
                }
                StaticFsNodeType::Directory(children) => {
                    inode.type_mode = crate::vfs::InodeTypeAndPerms::new_dir(0o777);
                    inode.size = children.len() as u64;
                }
            }
        }

        inode
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for StaticFsAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    fn partition_id(&self) -> uuid::Uuid {
        self.device_details.partition
    }

    async fn read(
        &self,
        inode: crate::vfs::InodeIndex,
        _f_handle_extra_data: &dyn FileHandleExtraData,
        _inode_extra_data: &dyn InodeExtraData,
        offset_bytes: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
        _blocking: bool,
    ) -> Result<(u64, FileReadResult), KernelError> {
        if size_bytes == 0 || buffer.is_empty() {
            return Ok((0, FileReadResult::Normal));
        }
        if size_bytes.div_ceil(4096) != buffer.len() as u64 {
            return kerror!(InvalidArgument);
        }

        let iter = StaticFsNodeIter::new(DOOM_FS_TREE);
        let mut node = None;
        for n in iter {
            if n.inode_index == inode {
                node = Some(n);
                break;
            }
        }
        let Some(node) = node else {
            return kerror!(InvalidArgument);
        };

        let data = match &node.node_type {
            StaticFsNodeType::File(data) => data,
            StaticFsNodeType::Directory(_) => return kerror!(InvalidArgument),
        };

        for (i, &phys_addr) in buffer.iter().enumerate() {
            let offset = i as u64 * 4096;
            if offset >= size_bytes {
                break;
            }
            let source_offset = offset_bytes + offset;
            let bytes_to_copy = (size_bytes - offset).min(4096);
            let src_ptr = data.as_ptr().wrapping_add(source_offset as usize);
            let dst_ptr = VirtAddr::from(phys_addr).0 as *mut u8;
            unsafe {
                std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, bytes_to_copy as usize);
            }
        }

        Ok((size_bytes, FileReadResult::Normal))
    }

    async fn read_dir(
        &self,
        inode: crate::vfs::InodeIndex,
        _inode_extra_data: &dyn InodeExtraData,
    ) -> Result<Box<[DirEntry]>, KernelError> {
        let iter = StaticFsNodeIter::new(DOOM_FS_TREE);
        let mut node = None;
        for n in iter {
            if n.inode_index == inode {
                node = Some(n);
                break;
            }
        }
        let Some(node) = node else {
            return kerror!(InvalidArgument);
        };

        let children = match &node.node_type {
            StaticFsNodeType::Directory(children) => children,
            StaticFsNodeType::File(_) => return kerror!(InvalidArgument),
        };

        let dir_entries: Vec<DirEntry> = children
            .iter()
            .map(|child| DirEntry {
                inode: child.inode_index,
                name: child.name.to_string().into_boxed_str(),
            })
            .collect();

        Ok(dir_entries.into_boxed_slice())
    }

    async fn write(
        &self,
        _inode: crate::vfs::InodeIndex,
        _f_handle_extra_data: &dyn FileHandleExtraData,
        _inode_extra_data: &dyn InodeExtraData,
        _offset: u64,
        _size: u64,
        _buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), KernelError> {
        kerror!(UnsupportedOperation)
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex, _parent: InodeIndex) -> Result<Inode, KernelError> {
        Ok(self.get_inode(inode))
    }
}
