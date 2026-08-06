use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    error::KernelError,
    kerror, kerror_unwrapped, lock_w_info, println, r_lock_w_info,
    sync::no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
    vec::Vec,
};

use crate::vfs::{DeviceId, GLOBAL_MOUNTS, InodeIdentifier, InodeIdentifierChain, ResolvedPathBorrowed, VFS};

static INODE_CACHE: NoIntSpinlock<FsTreeCache> = NoIntSpinlock::new(FsTreeCache::new());

#[derive(Debug)]
struct FsTreeNode {
    children: Vec<(Box<str>, InodeIdentifier)>,
}

/// Cache container for directory inode related data (fs tree, global mount points)
pub(super) struct FsTreeCache {
    inodes: BTreeMap<InodeIdentifier, FsTreeNode>,
    root: InodeIdentifier,
}

impl FsTreeCache {
    pub const fn new() -> Self {
        FsTreeCache {
            inodes: BTreeMap::new(),
            root: InodeIdentifier {
                device_id: DeviceId::new(0),
                index: 0,
            },
        }
    }
}

pub(super) fn init(root: InodeIdentifier) {
    let mut cache = lock_w_info!(INODE_CACHE);

    cache.inodes.clear();
    cache.inodes.insert(root, FsTreeNode { children: Vec::new() });
    cache.root = root;
}

/// Returns the inode chain for the given path.
/// If a `from` chain is provided, the path is relative and starts from there. From chain is
/// included in the returned chain. If no `from` chain is provided, the path is absolute and starts
/// from the root inode.
/// ret.0 is the last inode in the chain, the actual inode for this path
/// ret.1 is the chain going from root to this same last inode, with it included
pub(super) async fn get_inode_chain(
    path: ResolvedPathBorrowed<'_>,
    from: Option<InodeIdentifierChain>,
    namespace_mounts: Option<&BTreeMap<InodeIdentifier, InodeIdentifier>>,
) -> Result<(InodeIdentifier, InodeIdentifierChain), KernelError> {
    let mut chain = match from {
        Some(chain) => chain.to_vec(),
        None => Vec::new(),
    };
    if chain.is_empty() {
        let cache = lock_w_info!(INODE_CACHE);
        chain.push(cache.root);
    }

    let mut current = *chain.last().expect("chain should have at least 1 element");

    for component in path.iter() {
        if **component == *".." {
            if chain.len() > 1 {
                chain.pop();
                current = *chain.last().expect("chain should have at least 1 element");
            }
            continue;
        }

        let child = get_child(current, component, true, namespace_mounts).await?;
        chain.push(child);
        current = child;
    }

    Ok((current, chain.into_boxed_slice()))
}

/// Link a parent to a child with a given name. This function is technically optional, as if the
/// child is not found when searching, a scan of the parent dir will be performed to find the child.
/// For performance reasons, use this function
pub(super) fn link_inode(parent_cache_num: InodeIdentifier, name: Box<str>, inode_index: InodeIdentifier) {
    let mut cache = lock_w_info!(INODE_CACHE);
    let Some(parent_node) = cache.inodes.get_mut(&parent_cache_num) else {
        return;
    };

    parent_node.children.push((name, inode_index));
    cache.inodes.entry(inode_index).or_insert(FsTreeNode { children: Vec::new() });
}

/// Unlink a child from a parent. This function, unlike `link_inode`, is not optional. It must be
/// called to prevent incorrect path to inode resolution.
pub(super) fn unlink_inode(parent_cache_num: InodeIdentifier, name: &str) {
    let mut cache = lock_w_info!(INODE_CACHE);
    let Some(parent_node) = cache.inodes.get_mut(&parent_cache_num) else {
        return; //parent not in cache, nothing to unlink
    };

    let index = parent_node.children.iter().position(|(n, _)| n.as_ref() == name);
    if let Some(index) = index {
        parent_node.children.swap_remove(index);
    }
}

/// Removes an inode from the cache. This function is called when an inode is deleted.
/// Make sure to call `unlink_inode` before calling this function!!
pub(super) fn remove_inode(inode_index: InodeIdentifier) {
    let mut cache = lock_w_info!(INODE_CACHE);
    cache.inodes.remove(&inode_index);
}

pub(super) fn remove_device(_device_id: DeviceId) {
    let mut cache = lock_w_info!(INODE_CACHE);
    cache.inodes.clear(); //easiest and fastest ig :3
}

#[heap_future::heap_future]
pub async fn get_child(
    parent: InodeIdentifier,
    name: &str,
    resolve_mounts: bool,
    namespace_mounts: Option<&BTreeMap<InodeIdentifier, InodeIdentifier>>,
) -> Result<InodeIdentifier, KernelError> {
    let cache = lock_w_info!(INODE_CACHE);
    if let Some(parent_node) = cache.inodes.get(&parent) {
        if let Some((_, child)) = parent_node.children.iter().find(|(n, _)| n.as_ref() == name) {
            if !resolve_mounts {
                return Ok(*child);
            }
            return Ok(resolve_mount_point(*child, namespace_mounts).map_or_else(|| *child, |(_, overlaid)| overlaid));
        }
    }
    drop(cache);

    //load the directory, maybe parent was not loaded or child was just created

    let cache = load_dir(parent).await?;
    if let Some(parent_node) = cache.inodes.get(&parent) {
        if let Some((_, child)) = parent_node.children.iter().find(|(n, _)| n.as_ref() == name) {
            if !resolve_mounts {
                return Ok(*child);
            }
            return Ok(resolve_mount_point(*child, namespace_mounts).map_or_else(|| *child, |(_, overlaid)| overlaid));
        }
    }
    kerror!(NoEntry)
}

/// Loads the directory entries into the cache and returns the lock so cache isn't cleared before a
/// query is made
async fn load_dir(current: InodeIdentifier) -> Result<NoIntSpinlockGuard<'static, FsTreeCache>, KernelError> {
    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&current.device_id).ok_or(kerror_unwrapped!(NoEntry))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(NoEntry))?
        .clone();
    drop(vfs);

    let dir = fs.read_dir(current.index).await?;
    println!("load_dir: loaded directory for inode {:?}, entries: {}", current, dir.len());
    println!("load_dir: directory entries for inode {:?}: {:?}", current, dir);

    let mut children = Vec::new();
    if dir.is_empty() {
        return Ok(lock_w_info!(INODE_CACHE));
    }
    for dir_entry in dir.iter() {
        let inode_stat = fs.stat(dir_entry.inode, current.index).await;
        if let Err(e) = inode_stat {
            println!(level:error, "Failed to stat inode {} while loading directory: {e}", dir_entry.inode);
            continue;
        }
        let inode = unsafe { inode_stat.unwrap_unchecked() };

        let inode_index = InodeIdentifier {
            device_id: inode.device,
            index: inode.index,
        };

        let mut cache = lock_w_info!(INODE_CACHE);
        cache.inodes.insert(inode_index, FsTreeNode { children: Vec::new() });
        drop(cache);

        println!(
            "load_dir: added inode {:?} to cache for directory entry: {}",
            inode_index, dir_entry.name
        );
        children.push((dir_entry.name.clone(), inode_index));
    }

    println!(
        "load_dir: finished loading directory for inode {:?}, children count: {}",
        current,
        children.len()
    );

    let mut cache = lock_w_info!(INODE_CACHE);
    cache
        .inodes
        .entry(current)
        .or_insert(FsTreeNode { children: Vec::new() })
        .children = children;

    Ok(cache)
}

/// Resolves the mount point for the given inode. If there are no mounts on top if this inode, None
/// is returned. If there are mounts, the top 2 inodes on the stack are returned. First is the mount
/// point, second is the overlaid inode
pub fn resolve_mount_point(
    mut inode: InodeIdentifier,
    namespace_mounts: Option<&BTreeMap<InodeIdentifier, InodeIdentifier>>,
) -> Option<(InodeIdentifier, InodeIdentifier)> {
    let global_mounts = r_lock_w_info!(GLOBAL_MOUNTS);
    let mut prev_inode = None;

    loop {
        let mut found_mount = false;

        if let Some(mount) = global_mounts.get(&inode) {
            prev_inode = Some(inode);
            inode = *mount;
            found_mount = true;
        }

        if let Some(namespace_mounts) = namespace_mounts {
            if let Some(mount) = namespace_mounts.get(&inode) {
                prev_inode = Some(inode);
                inode = *mount;
                found_mount = true;
            }
        }

        if !found_mount {
            break;
        }
    }
    prev_inode.map(|prev| (prev, inode))
}
