use std::error::ErrorCode;
use std::{println, vec};
use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    lock_w_info, printlnc,
    sync::no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
    vec::Vec,
};

use super::{DeviceId, Inode, InodeIdentifier, InodeIdentifierChain, ResolvedPathBorrowed, VFS};

pub(super) static INODE_CACHE: NoIntSpinlock<InodeCache> = NoIntSpinlock::new(InodeCache::new());

#[derive(Debug)]
struct FsTreeNode {
    children: Vec<(Box<str>, InodeIdentifier)>,
}

pub(super) struct InodeCache {
    inodes: BTreeMap<InodeIdentifier, (Inode, FsTreeNode)>,
    root: InodeIdentifier,
    ///maps from parent inode in mount point to child inode in mount point
    mount_points: BTreeMap<InodeIdentifier, InodeIdentifier>,
}

impl InodeCache {
    pub const fn new() -> Self {
        InodeCache {
            inodes: BTreeMap::new(),
            root: InodeIdentifier {
                device_id: DeviceId::new(0),
                index: 0,
            },
            mount_points: BTreeMap::new(),
        }
    }
}

///Should be called when mounting a new fs as root
pub fn init(root: Inode) {
    let mut cache = lock_w_info!(INODE_CACHE);

    let inode_index = InodeIdentifier {
        device_id: root.device,
        index: root.index,
    };
    cache.inodes.clear();
    cache.inodes.insert(inode_index, (root, FsTreeNode { children: Vec::new() }));
    cache.root = inode_index;
}

pub fn get_inode(inode_index: InodeIdentifier) -> Option<Inode> {
    let cache = &mut lock_w_info!(INODE_CACHE);
    println!("inode cache: {:?}", cache.inodes.values().collect::<Vec<_>>());
    cache.inodes.get(&inode_index).map(|(inode, _)| inode).cloned()
}

pub async fn get_unmount_inodes(
    path: ResolvedPathBorrowed<'_>,
    from: Option<InodeIdentifier>,
) -> Result<(InodeIdentifier, InodeIdentifier), ErrorCode> {
    let mut cache = Some(lock_w_info!(INODE_CACHE));
    let mut current = from.unwrap_or(cache.as_ref().expect("is some").root);
    for component in path.iter() {
        while let Some(mount_point) = cache.as_ref().expect("is some").mount_points.get(&current) {
            if *mount_point == current {
                printlnc!((0, 0, 255), "Detected mount loop at inode {:?}\n", current);
                break;
            }
            current = *mount_point;
        }
        let child = find_child_no_mounts(current, component, &mut cache).await?;
        current = child;
    }
    let mut old = current;
    while let Some(mount_point) = cache.as_ref().expect("is some").mount_points.get(&current) {
        if *mount_point == current {
            printlnc!((0, 0, 255), "Detected mount loop at inode {:?}\n", current);
            break;
        }
        old = current;
        current = *mount_point;
    }
    if old == current {
        return Err(ErrorCode::NotMounted);
    }
    Ok((old, current))
}

pub async fn get_inode_chain(
    path: ResolvedPathBorrowed<'_>,
    from: Option<InodeIdentifierChain>,
) -> Result<(InodeIdentifier, InodeIdentifierChain), ErrorCode> {
    let mut cache_lock = Some(lock_w_info!(INODE_CACHE));
    let mut current = from
        .unwrap_or(Box::new([cache_lock.as_ref().expect("is some").root]))
        .to_vec();
    if current.is_empty() {
        current = vec![cache_lock.as_ref().expect("is some").root];
    }
    for component in path.iter() {
        if **component == *".." {
            if current.len() > 1 {
                current.pop();
            }
            continue;
        }

        let current_last = *current.last().expect("current can't be empty");
        while let Some(mount_point) = cache_lock.as_ref().expect("is some").mount_points.get(&current_last) {
            if *mount_point == current_last {
                printlnc!((0, 255, 255), "Detected mount loop at inode {:?}\n", current);
                break;
            }
            *current.last_mut().expect("current can't be empty") = *mount_point;
        }

        let child = find_child_no_mounts(*current.last().expect("current can't be empty"), component, &mut cache_lock).await?;
        current.push(child);
    }
    if let Some(mount_point) = cache_lock
        .expect("is some")
        .mount_points
        .get(current.last().expect("current can't be empty"))
    {
        *current.last_mut().expect("current can't be empty") = *mount_point;
    }
    let file = *current.last().expect("current can't be empty");
    if current.len() > 1 {
        current.pop();
    }

    Ok((file, current.into_boxed_slice()))
}

async fn find_child_no_mounts(
    current: InodeIdentifier,
    f_name: &str,
    cache: &mut Option<NoIntSpinlockGuard<'_, InodeCache>>,
) -> Result<InodeIdentifier, ErrorCode> {
    let current_node = cache
        .as_ref()
        .expect("is some")
        .inodes
        .get(&current)
        .ok_or(ErrorCode::InodeNotPresent)?;
    let child = current_node.1.children.iter().find(|(name, _)| **name == *f_name);
    if let Some(child) = child {
        return Ok(child.1);
    }
    // If the child is not found, we need to load the directory
    load_dir(current, cache).await?;
    // After loading, we check again
    let current_node = cache
        .as_ref()
        .expect("is some")
        .inodes
        .get(&current)
        .ok_or(ErrorCode::InodeNotPresent)?;
    let child = current_node.1.children.iter().find(|(name, _)| **name == *f_name);
    if let Some(child) = child {
        return Ok(child.1);
    }
    Err(ErrorCode::InodeNotPresent)
}

async fn load_dir(current: InodeIdentifier, cache: &mut Option<NoIntSpinlockGuard<'_, InodeCache>>) -> Result<(), ErrorCode> {
    let inode = cache
        .as_ref()
        .expect("is some")
        .inodes
        .get(&current)
        .ok_or(ErrorCode::InodeNotPresent)?;
    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.0.device).ok_or(ErrorCode::NoEntry)?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(ErrorCode::NoEntry)?
        .clone();
    drop(vfs);
    drop(cache.take()); //drop lock

    let dir = fs.read_dir(current.index).await?;

    let mut children = Vec::new();
    if dir.is_empty() {
        return Ok(());
    }
    for dir_entry in dir.iter() {
        drop(cache.take()); //drop in loop
        let inode = fs.stat(dir_entry.inode).await;
        let inode_index = InodeIdentifier {
            device_id: inode.device,
            index: inode.index,
        };
        *cache = Some(lock_w_info!(INODE_CACHE)); //get lock back
        cache
            .as_mut()
            .expect("is some")
            .inodes
            .insert(inode_index, (inode, FsTreeNode { children: Vec::new() }));
        children.push((dir_entry.name.clone(), inode_index));
    }
    cache
        .as_mut()
        .expect("is some")
        .inodes
        .get_mut(&current)
        .ok_or(ErrorCode::InodeNotPresent)?
        .1
        .children = children;
    Ok(())
}

pub fn update_inode(cache_num: InodeIdentifier, inode: Inode) -> Result<(), ErrorCode> {
    let mut cache = lock_w_info!(INODE_CACHE);
    cache.inodes.get_mut(&cache_num).ok_or(ErrorCode::InodeNotPresent)?.0 = inode;
    Ok(())
}

pub fn insert_inode(parent_cache_num: InodeIdentifier, name: Box<str>, inode: Inode) -> Result<(), ErrorCode> {
    let mut cache = lock_w_info!(INODE_CACHE);
    let inode_index = InodeIdentifier {
        device_id: inode.device,
        index: inode.index,
    };
    cache.inodes.insert(inode_index, (inode, FsTreeNode { children: Vec::new() }));
    let parent_res = cache.inodes.get_mut(&parent_cache_num);
    match parent_res {
        None => {
            cache.inodes.remove(&inode_index);
            return Err(ErrorCode::InodeNotPresent);
        },
        Some(parent) => parent.1.children.push((name, inode_index)),
    }
    Ok(())
}

///parent_cache_num refers to the mountpoint itself, on top of which the new inode will be mounted
pub fn mount_inode(parent_cache_num: InodeIdentifier, inode: Inode) {
    let mut cache = lock_w_info!(INODE_CACHE);
    let inode_index = InodeIdentifier {
        device_id: inode.device,
        index: inode.index,
    };
    cache.inodes.insert(inode_index, (inode, FsTreeNode { children: Vec::new() }));
    cache.mount_points.insert(parent_cache_num, inode_index);
}

///parent_cache_num refers to the parent directory, NOT the mountpoint itself
///returns true if the last mountpoint of this filesystem was unmounted
pub fn unmount_inode(parent_cache_num: InodeIdentifier) -> bool {
    let mut cache = lock_w_info!(INODE_CACHE);
    let unmounted_device = cache
        .mount_points
        .remove(&parent_cache_num)
        .map_or(DeviceId::new(u64::MAX), |v| v.device_id);
    let count = cache
        .mount_points
        .values()
        .filter(|&&v| v.device_id == unmounted_device)
        .count();
    drop(cache);
    if count == 0 {
        remove_device(unmounted_device);
        return true;
    }
    false
}

/// Removes all inodes associated with a specific device ID. Called when device is fully unmounted
pub fn remove_device(device_id: DeviceId) {
    let mut cache = lock_w_info!(INODE_CACHE);
    cache.inodes.retain(|_, (inode, _)| inode.device != device_id);
    if cache.root.device_id == device_id {
        cache.root = InodeIdentifier {
            device_id: DeviceId::new(0),
            index: 0,
        };
    }
}
