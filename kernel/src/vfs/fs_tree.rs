use std::error::ErrorCode;
use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    lock_w_info, printlnc,
    sync::no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
    vec::Vec,
};
use std::{println, vec};

use super::{DeviceId, InodeIdentifier, InodeIdentifierChain, ResolvedPathBorrowed, VFS};

pub(super) static INODE_CACHE: NoIntSpinlock<InodeCache> = NoIntSpinlock::new(InodeCache::new());

#[derive(Debug)]
struct FsTreeNode {
    children: Vec<(Box<str>, InodeIdentifier)>,
}

pub(super) struct InodeCache {
    inodes: BTreeMap<InodeIdentifier, FsTreeNode>,
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
pub fn init(root: InodeIdentifier) {
    let mut cache = lock_w_info!(INODE_CACHE);

    cache.inodes.clear();
    cache.inodes.insert(root, FsTreeNode { children: Vec::new() });
    cache.root = root;
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
        println!(
            "get_inode_chain: current last inode: {:?}, component: {}",
            current_last, component
        );

        while let Some(mount_point) = cache_lock.as_ref().expect("is some").mount_points.get(&current_last) {
            if *mount_point == current_last {
                printlnc!((0, 255, 255), "Detected mount loop at inode {:?}\n", current);
                break;
            }
            *current.last_mut().expect("current can't be empty") = *mount_point;
            println!("get_inode_chain: following mount point to inode: {:?}", *mount_point);
        }

        let child = find_child_no_mounts(*current.last().expect("current can't be empty"), component, &mut cache_lock).await?;
        println!("get_inode_chain: found child inode: {:?} for component: {}", child, component);
        current.push(child);
    }
    while let Some(mount_point) = cache_lock
        .as_ref()
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

    println!(
        "find_child_no_mounts: current node: {:?}, looking for child: {}",
        current, f_name
    );

    let child = current_node.children.iter().find(|(name, _)| **name == *f_name);
    if let Some(child) = child {
        println!(
            "find_child_no_mounts: found child in cache: {:?} for name: {}",
            child.1, f_name
        );
        return Ok(child.1);
    }
    // If the child is not found, we need to load the directory
    println!(
        "find_child_no_mounts: child not found in cache for name: {}, loading directory for inode: {:?}",
        f_name, current
    );
    load_dir(current, cache).await?;
    // After loading, we check again
    let current_node = cache
        .as_ref()
        .expect("is some")
        .inodes
        .get(&current)
        .ok_or(ErrorCode::InodeNotPresent)?;
    let child = current_node.children.iter().find(|(name, _)| **name == *f_name);
    if let Some(child) = child {
        println!(
            "find_child_no_mounts: found child after loading directory: {:?} for name: {}",
            child.1, f_name
        );
        return Ok(child.1);
    }
    println!(
        "find_child_no_mounts: child still not found after loading directory for name: {}, inode: {:?}",
        f_name, current
    );
    //print dir children

    for child in &current_node.children {
        println!("find_child_no_mounts: directory entry: {} with inode: {:?}", child.0, child.1);
    }

    Err(ErrorCode::NoEntry)
}

async fn load_dir(current: InodeIdentifier, cache: &mut Option<NoIntSpinlockGuard<'_, InodeCache>>) -> Result<(), ErrorCode> {
    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&current.device_id).ok_or(ErrorCode::NoEntry)?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(ErrorCode::NoEntry)?
        .clone();
    drop(vfs);
    drop(cache.take()); //drop lock

    let dir = fs.read_dir(current.index).await?;
    println!("load_dir: loaded directory for inode {:?}, entries: {}", current, dir.len());
    println!("load_dir: directory entries for inode {:?}: {:?}", current, dir);

    let mut children = Vec::new();
    if dir.is_empty() {
        *cache = Some(lock_w_info!(INODE_CACHE)); //get lock back
        return Ok(());
    }
    for dir_entry in dir.iter() {
        drop(cache.take()); //drop in loop
        let inode_stat = fs.stat(dir_entry.inode, current.index).await;
        if let Err(e) = inode_stat {
            println!(level:error, "Failed to stat inode {} while loading directory: {e}", dir_entry.inode);
            *cache = Some(lock_w_info!(INODE_CACHE)); //get lock back
            continue;
        }
        let inode = unsafe { inode_stat.unwrap_unchecked() };

        let inode_index = InodeIdentifier {
            device_id: inode.device,
            index: inode.index,
        };
        *cache = Some(lock_w_info!(INODE_CACHE)); //get lock back
        cache
            .as_mut()
            .expect("is some")
            .inodes
            .insert(inode_index, FsTreeNode { children: Vec::new() });
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

    cache
        .as_mut()
        .expect("is some")
        .inodes
        .get_mut(&current)
        .ok_or(ErrorCode::InodeNotPresent)?
        .children = children;

    Ok(())
}

pub fn insert_inode(parent_cache_num: InodeIdentifier, name: Box<str>, inode_index: InodeIdentifier) -> Result<(), ErrorCode> {
    let mut cache = lock_w_info!(INODE_CACHE);
    cache.inodes.insert(inode_index, FsTreeNode { children: Vec::new() });
    let parent_res = cache.inodes.get_mut(&parent_cache_num);
    match parent_res {
        None => {
            cache.inodes.remove(&inode_index);
            return Err(ErrorCode::InodeNotPresent);
        }
        Some(parent) => parent.children.push((name, inode_index)),
    }
    Ok(())
}

pub fn unlink_inode(parent_cache_num: InodeIdentifier, name: &str) -> Result<(), ErrorCode> {
    let mut cache = lock_w_info!(INODE_CACHE);
    let parent_res = cache.inodes.get_mut(&parent_cache_num);
    match parent_res {
        None => Err(ErrorCode::InodeNotPresent),
        Some(parent) => {
            if let Some(pos) = parent.children.iter().position(|(child_name, _)| **child_name == *name) {
                let inode_index = parent.children[pos].1;
                parent.children.remove(pos);
                cache.inodes.remove(&inode_index);
                Ok(())
            } else {
                Err(ErrorCode::NoEntry)
            }
        }
    }
}

///parent_cache_num refers to the mountpoint itself, on top of which the new inode will be mounted
pub fn mount_inode(parent_cache_num: InodeIdentifier, target_inode: InodeIdentifier) {
    let mut cache = lock_w_info!(INODE_CACHE);
    cache.mount_points.insert(parent_cache_num, target_inode);
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
    cache.inodes.retain(|inode, _| inode.device_id != device_id);
    if cache.root.device_id == device_id {
        cache.root = InodeIdentifier {
            device_id: DeviceId::new(0),
            index: 0,
        };
    }
}

pub fn get_child_inode(parent_cache_num: InodeIdentifier, name: &str) -> Result<InodeIdentifier, ErrorCode> {
    let cache = lock_w_info!(INODE_CACHE);
    let parent_res = cache.inodes.get(&parent_cache_num);
    match parent_res {
        None => Err(ErrorCode::InodeNotPresent),
        Some(parent) => parent
            .children
            .iter()
            .find(|(child_name, _)| **child_name == *name)
            .map(|(_, inode_index)| *inode_index)
            .ok_or(ErrorCode::NoEntry),
    }
}
