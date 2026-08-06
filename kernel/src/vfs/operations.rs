use core::sync::atomic::{AtomicU64, Ordering};
use std::{
    boxed::Box,
    error::KernelError,
    kerror, kerror_unwrapped, lock_w_info, println, printlnc,
    string::ToString,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlockGuard},
    vec::Vec,
    w_lock_w_info,
};

use uuid::Uuid;

use crate::{
    drivers::{
        block_device::disk::{BlockDevice, DirEntry, MountedPartition, PartitionSchemeDriver},
        gpt::GPTDriver,
    },
    memory::addresses::PhysAddr,
    vfs::{
        GLOBAL_MOUNTS, Inode, InodeIdentifier,
        file::{OpenFlags, get_file},
    },
};

use super::{
    DeviceDetails, InodeIdentifierChain, InodeTypeAndPerms, ROOT_INODE_INDEX, ResolvedPath, ResolvedPathBorrowed, VFS, Vfs,
    file::{FileFlags, FileHandle},
    filesystem_trait::FileSystem,
    fs_tree::{self},
};

pub async fn add_disk(disk: Arc<dyn BlockDevice>) -> Uuid {
    //for now only GPT
    let gpt_driver = GPTDriver {};
    let disk_uuid = gpt_driver.guid(disk.get()).await;
    let partitions = gpt_driver.partitions(disk.get()).await;
    let partition_guids: Vec<Uuid> = partitions.iter().map(|(guid, _)| *guid).collect();

    let mut vfs = lock_w_info!(VFS);

    vfs.disks.insert(disk_uuid, (disk, partition_guids));

    for partition in partitions {
        let device = partition.1.device;
        vfs.available_partitions.insert(partition.0, partition.1);
        vfs.devices.insert(
            device,
            DeviceDetails {
                drive: disk_uuid,
                partition: partition.0,
            },
        );
    }

    disk_uuid
}

//called after unmounting all partitions or when it was forcibly removed
fn remove_disk(uuid: Uuid) {
    let mut vfs = lock_w_info!(VFS);
    let Some(partitions) = vfs.disks.remove(&uuid) else {
        //slow path, maybe disk was forcibly removed
        remove_disk_slow(uuid, vfs);
        return;
    };
    for partition in partitions.1.iter() {
        let part = vfs.available_partitions.remove(partition);
        if let Some(part) = part {
            let was_mounted = vfs.mounted_filesystems.remove(partition);
            let had_device = vfs.devices.remove(&part.device);
            if was_mounted.is_some() {
                printlnc!(level:error, (0, 255, 255), "Inconsistent VFS state detected when removing disk: had mounted partitions");
            }
            if had_device.is_none() {
                printlnc!(level:error, (0, 255, 255), "Inconsistent VFS state detected when removing disk: missing device for partition {}", partition);
            }
        } else {
            printlnc!(level:error, (0, 255, 255), "Inconsistent VFS state detected when removing disk: missing partition {}", partition);
        }
    }
}

fn remove_disk_slow(uuid: Uuid, mut vfs: NoIntSpinlockGuard<'_, Vfs>) {
    printlnc!(level:warn, (0, 255, 255), "Warning: attempting to remove non existent disk {}", uuid);
    let Vfs {
        available_partitions,
        devices,
        mounted_filesystems,
        ..
    } = &mut *vfs;
    available_partitions.retain(|part_id, part| {
        let device = devices.get(&part.device);
        if device.is_none() {
            printlnc!(level:error, (0, 255, 255), "Inconsistent VFS state detected when removing disk: device is none");
        }
        let retain = if let Some(device) = device {
            device.drive != uuid
        } else {
            false //no device behind a partition, remove it
        };
        if retain {
            return true;
        }
        let was_mounted = mounted_filesystems.remove(part_id);
        if was_mounted.is_some() {
            printlnc!(level:error, (0, 255, 255), "Inconsistent VFS state detected when removing disk: had mounted partitions");
        }
        false
    });
}

#[heap_future::heap_future]
pub async fn mount_blkdev_partition(part_id: Uuid, mountpoint: ResolvedPath) -> Result<(), KernelError> {
    let mut vfs = lock_w_info!(VFS);
    let Some(partition) = vfs.available_partitions.get(&part_id) else {
        return kerror!(NoEntry);
    };
    let partition = partition.clone();

    let Some(device_detail) = vfs.devices.get(&partition.device) else {
        return kerror!(InternalFSError);
    };
    let drive_id = device_detail.drive;
    let Some(disk) = vfs.disks.get_mut(&drive_id) else {
        return kerror!(NoEntry);
    };
    let disk_cloned = disk.0.clone();

    println!(level:info, "Mounting partition with fs type {}", partition.fs_type);
    vfs.print_available_fs_driver_types();

    let Some(fs_factory) = vfs.filesystem_driver_factories.get(&partition.fs_type).cloned() else {
        return kerror!(UnsupportedFilesystem);
    };
    drop(vfs);

    let mounted_partition = MountedPartition {
        disk: disk_cloned,
        partition,
    };
    let fs = fs_factory.mount(mounted_partition).await;
    if let Err(e) = mount_filesystem(mountpoint, fs.clone(), part_id).await {
        let _ = fs.unmount().await; //double error...???
        Err(e)
    } else {
        Ok(())
    }
}

///Not dealing with namespace mounts yet, all is global
#[heap_future::heap_future]
async fn mount_filesystem(mountpoint: ResolvedPath, fs: Arc<dyn FileSystem + Send>, part_id: Uuid) -> Result<(), KernelError> {
    let root = mountpoint.inner().is_empty();
    println!("Mounting filesystem at {:?}", mountpoint.inner());

    if root {
        make_new_root_checks(&fs).await?;
        mount_vfs_adapters(&fs).await;
    }

    let (parent_inode, _parent_inode_chain) = fs_tree::get_inode_chain((&mountpoint).into(), None, None).await?;
    let inode_id = InodeIdentifier {
        device_id: fs.device_id(),
        index: ROOT_INODE_INDEX,
    };

    let mut vfs = lock_w_info!(VFS);
    vfs.mounted_filesystems.insert(part_id, fs);
    mount_inode(parent_inode, inode_id);

    drop(vfs);

    Ok(())
}

async fn make_new_root_checks(fs: &Arc<dyn FileSystem + Send>) -> Result<(), KernelError> {
    let inode_id = InodeIdentifier {
        device_id: fs.device_id(),
        index: ROOT_INODE_INDEX,
    };
    fs_tree::init(inode_id);

    //root checks
    let root_dirs = fs.read_dir(ROOT_INODE_INDEX).await?;
    let required_dirs = ["tty", "proc", "net"];
    for required_dir in required_dirs.iter() {
        if !root_dirs.iter().any(|entry| entry.name.as_ref() == *required_dir) {
            println!("Root filesystem is missing required directory {required_dir}, creating it");
            //create the required directory
            fs.create(required_dir, ROOT_INODE_INDEX, InodeTypeAndPerms::new_dir(0o755), 0, 0)
                .await?;
        }
    }
    Ok(())
}

fn mount_inode(parent_inode: InodeIdentifier, child_inode: InodeIdentifier) {
    let mut global_mounts = w_lock_w_info!(GLOBAL_MOUNTS);
    global_mounts.insert(parent_inode, child_inode);
    //any other updates
}

fn unmount_inode(parent_inode: InodeIdentifier) {
    let mut global_mounts = w_lock_w_info!(GLOBAL_MOUNTS);
    global_mounts.remove(&parent_inode);
    //any other updates
}

async fn mount_vfs_adapters(fs: &Arc<dyn FileSystem + Send>) {
    let proc_adapter = crate::vfs::adapters::ProcAdapter::get();
    let proc_adapter_partition_id = proc_adapter.partition_id();
    let tty_adapter = crate::vfs::adapters::TtyAdapter::get();
    let tty_adapter_partition_id = tty_adapter.partition_id();

    let adapters = [
        ("tty", tty_adapter, tty_adapter_partition_id),
        ("proc", proc_adapter, proc_adapter_partition_id),
    ];

    let dir_entries = fs.read_dir(ROOT_INODE_INDEX).await.expect("Failed to read root dir");

    for (mountpoint, adapter, _partition_id) in adapters.iter() {
        let entry = dir_entries
            .iter()
            .find(|entry| entry.name.as_ref() == *mountpoint)
            .expect("should have created dirs for root fs");
        let inode_id = InodeIdentifier {
            device_id: adapter.device_id(),
            index: entry.inode,
        };
        mount_inode(
            InodeIdentifier {
                device_id: fs.device_id(),
                index: entry.inode,
            },
            inode_id,
        );
    }
}

pub async fn unmount(path: ResolvedPathBorrowed<'_>) -> Result<(), KernelError> {
    if path.inner().is_empty() {
        unimplemented!("Unmounting root filesystem is not supported yet");
        //TODO: update fs_tree root
    }

    let path_len = path.inner().len();
    let without_last = if path_len > 1 { path.index(0..path_len - 1) } else { path };
    let (parent_inode, _parent_chain) = fs_tree::get_inode_chain(without_last, None, None).await?;

    let inode = fs_tree::get_child(
        parent_inode,
        path.get(path_len - 1)
            .ok_or(kerror_unwrapped!(InvalidOperation))? //TODO: fix
            .to_string()
            .as_str(),
        false,
        None,
    )
    .await?;

    let inodes = fs_tree::resolve_mount_point(inode, None).ok_or(kerror_unwrapped!(NoEntry))?;

    unmount_inode(inodes.0);

    // if last_part_mount {
    //     let mut vfs = lock_w_info!(VFS);
    //     let Some(device) = vfs.devices.get(&inodes.1.device_id) else {
    //         return Ok(());
    //     };
    //     let partition_id = device.partition;
    //     let Some(partition) = vfs.mounted_filesystems.remove(&partition_id) else {
    //         return Ok(());
    //     };
    //     partition.unmount().await?;
    // }
    todo!("Figure out unmounting whole partitions/devices. Checks need to happen BEFORE main unmount code");
}

#[heap_future::heap_future]
pub async fn open_file(
    path: ResolvedPathBorrowed<'_>,
    from: Option<InodeIdentifierChain>,
    open_flags: OpenFlags,
) -> Result<FileHandle, KernelError> {
    if open_flags.truncate() {
        println!(level:error, "Truncate on open is not supported yet");
        return kerror!(InvalidOperation);
    }

    let (inode_index, inode_chain) = fs_tree::get_inode_chain(path, from, None).await?;
    let chain_len = inode_chain.len();
    let parent_in_chain_index = if chain_len == 1 { 0 } else { chain_len - 2 };
    let open_file = get_file(inode_index, inode_chain[parent_in_chain_index]).await?;
    let is_dir = unsafe { open_file.inode.get_read_ptr().type_mode.is_dir() };
    let file_flags = FileFlags::new_with_flags(open_flags.read(), open_flags.write(), open_flags.append(), is_dir);
    //TODO: check permissions
    Ok(FileHandle {
        inode: inode_index,
        parent_chain: inode_chain,
        position: AtomicU64::new(0),
        file_flags,
        open_file,
    })
}

pub async fn get_dir_entries(file_handle: &FileHandle) -> Result<Box<[DirEntry]>, KernelError> {
    let inode = unsafe { file_handle.open_file.inode.get_read_ptr() };

    if !inode.type_mode.is_dir() {
        println!("file {:?} is not a directory", file_handle.inode);
        return kerror!(UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.device).ok_or(kerror_unwrapped!(NoEntry))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(NoEntry))?;
    let fs = fs.clone();
    drop(vfs);
    fs.read_dir(file_handle.inode.index).await
}

pub async fn create_file(parent_dir: &FileHandle, name: &str, inode_type: InodeTypeAndPerms) -> Result<(), KernelError> {
    if !parent_dir.file_flags.write() {
        return kerror!(InsufficientPermissions);
    }
    if !parent_dir.file_flags.dir() {
        return kerror!(UnsupportedOperation);
    }

    let mut parent_inode = parent_dir.open_file.inode.lock().await;
    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs
        .devices
        .get(&parent_inode.device)
        .ok_or(kerror_unwrapped!(InodeNotPresent))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(InodeNotPresent))?;
    let fs = fs.clone();
    drop(vfs);

    let (new_parent_inode, file_inode) = fs.create(name, parent_inode.index, inode_type, 0, 0).await?;
    println!(
        "create file returned file and parent inodes: {:X?}, {:X?}",
        file_inode, new_parent_inode
    );
    let child_id = InodeIdentifier {
        device_id: new_parent_inode.device,
        index: file_inode.index,
    };
    fs_tree::link_inode(parent_dir.inode, name.to_string().into_boxed_str(), child_id);
    parent_inode.update_from(&new_parent_inode);

    Ok(())
}

pub async fn link_file(parent_dir: &FileHandle, name: &str, target: &FileHandle) -> Result<(), KernelError> {
    if !parent_dir.file_flags.write() {
        return kerror!(InsufficientPermissions);
    }
    if !parent_dir.file_flags.dir() {
        return kerror!(UnsupportedOperation);
    }

    let mut parent_inode = parent_dir.open_file.inode.lock().await;
    let target_inode = unsafe { target.open_file.inode.get_read_ptr() };

    let parent_device = parent_inode.device;
    let target_device = target_inode.device;

    if parent_device != target_device {
        return kerror!(UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs
        .devices
        .get(&parent_inode.device)
        .ok_or(kerror_unwrapped!(InodeNotPresent))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(NoEntry))?;
    let fs = fs.clone();
    drop(vfs);

    let (new_parent_inode, _new_child_inode) = fs.link(target_inode.index, parent_inode.index, name).await?;
    println!("link file returned new parent inode: {:X?}", new_parent_inode);
    let child_id = InodeIdentifier {
        device_id: parent_inode.device,
        index: target_inode.index,
    };
    fs_tree::link_inode(parent_dir.inode, name.to_string().into_boxed_str(), child_id);
    parent_inode.update_from(&new_parent_inode);
    target.open_file.inode.lock().await.link_cnt += 1;

    Ok(())
}

pub async fn unlink_file(parent_dir: &FileHandle, name: &str) -> Result<(), KernelError> {
    if !parent_dir.file_flags.write() {
        return kerror!(InsufficientPermissions);
    }
    if !parent_dir.file_flags.dir() {
        return kerror!(UnsupportedOperation);
    }

    let mut parent_inode = parent_dir.open_file.inode.lock().await;

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs
        .devices
        .get(&parent_inode.device)
        .ok_or(kerror_unwrapped!(InodeNotPresent))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(NoEntry))?;
    let fs = fs.clone();
    drop(vfs);

    let (new_parent_inode, new_child_inode) = fs.unlink(parent_inode.index, name).await?;
    //ignore error, at most there's no entry, but that shouldn't matter too much
    fs_tree::unlink_inode(parent_dir.inode, name);

    parent_inode.update_from(&new_parent_inode);
    let child_file = get_file(
        InodeIdentifier {
            device_id: parent_inode.device,
            index: new_child_inode.index,
        },
        InodeIdentifier {
            device_id: parent_inode.device,
            index: parent_inode.index,
        },
    )
    .await?;
    let mut child_inode = child_file.inode.lock().await;
    child_inode.link_cnt -= 1;

    Ok(())
}

pub async fn write_file(file_handle: &FileHandle, buffer: &[PhysAddr], size: u64) -> Result<u64, KernelError> {
    let mut inode = file_handle.open_file.inode.lock().await;

    let desired_offset = if file_handle.file_flags.append() {
        inode.size
    } else {
        file_handle.position.load(Ordering::Relaxed)
    };

    if !file_handle.file_flags.write() {
        return kerror!(InsufficientPermissions);
    }

    if inode.type_mode.is_dir() {
        return kerror!(UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.device).ok_or(kerror_unwrapped!(NoEntry))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(NoEntry))?;
    let fs = fs.clone();
    drop(vfs);

    let res = fs.write(inode.index, desired_offset, size, buffer).await?;

    println!("operations::write_file: Wrote {} bytes", res.1);

    file_handle
        .position
        .store(res.1.min(size) + desired_offset, Ordering::Relaxed);

    inode.update_from(&res.0);

    Ok(res.1)
}

pub async fn stat_file(file_handle: &FileHandle) -> Inode {
    file_handle.open_file.inode.lock().await.clone()
}

pub async fn read_file(file_handle: &FileHandle, buffer: &[PhysAddr], size: u64) -> Result<u64, KernelError> {
    if !file_handle.file_flags.read() {
        return kerror!(InsufficientPermissions);
    }

    let offset = file_handle.position.load(Ordering::Relaxed);

    let inode = unsafe { file_handle.open_file.inode.get_read_ptr() };

    if inode.type_mode.is_dir() {
        return kerror!(UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.device).ok_or(kerror_unwrapped!(NoEntry))?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(kerror_unwrapped!(NoEntry))?;
    let fs = fs.clone();
    drop(vfs);

    let bytes_read = fs.read(inode.index, offset, size, buffer).await?;

    println!("operations::read_file: Read {} bytes", bytes_read);

    let res = bytes_read.min(size);
    file_handle.position.store(offset + res, Ordering::Relaxed);
    Ok(res)
}
