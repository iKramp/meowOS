use std::{
    boxed::Box,
    error::ErrorCode,
    lock_w_info,
    mem_utils::PhysAddr,
    println, printlnc,
    string::ToString,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlockGuard},
    vec::Vec,
};

use uuid::Uuid;

use crate::{
    drivers::{
        block_device::disk::{BlockDevice, DirEntry, MountedPartition, PartitionSchemeDriver},
        gpt::GPTDriver,
    },
    vfs::{Inode, InodeIdentifier, file::get_file},
};

use super::{
    DeviceDetails, InodeIdentifierChain, InodeType, ROOT_INODE_INDEX, ResolvedPath, ResolvedPathBorrowed, VFS,
    VFS_ADAPTER_DEVICE, Vfs,
    file::{FileFlags, FileHandle},
    filesystem_trait::FileSystem,
    fs_tree::{self},
    resolve_path,
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

pub async fn mount_blkdev_partition(part_id: Uuid, mountpoint: ResolvedPath) -> Result<(), ErrorCode> {
    let mut vfs = lock_w_info!(VFS);
    let Some(partition) = vfs.available_partitions.get(&part_id) else {
        return Err(ErrorCode::NoEntry);
    };
    let partition = partition.clone();

    let Some(device_detail) = vfs.devices.get(&partition.device) else {
        return Err(ErrorCode::InternalFSError);
    };
    let drive_id = device_detail.drive;
    let Some(disk) = vfs.disks.get_mut(&drive_id) else {
        return Err(ErrorCode::NoEntry);
    };
    let disk_cloned = disk.0.clone();

    let Some(fs_factory) = vfs.filesystem_driver_factories.get(&partition.fs_type).cloned() else {
        return Err(ErrorCode::UnsupportedFilesystem);
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

async fn mount_filesystem(mountpoint: ResolvedPath, fs: Arc<dyn FileSystem + Send>, part_id: Uuid) -> Result<(), ErrorCode> {
    let root = mountpoint.inner().is_empty();
    if root {
        //mounting root
        println!("Mounting root filesystem");
        mount_new_root(&fs).await?;
        let fs: Arc<dyn FileSystem + Send> = fs;
        let mut vfs = lock_w_info!(VFS);
        vfs.mounted_filesystems.insert(part_id, fs);
        mount_vfs_adapters(vfs).await;
    } else {
        println!("Mounting filesystem at {:?}", mountpoint.inner());
        let (parent_inode, _parent_inode_chain) = fs_tree::get_inode_chain((&mountpoint).into(), None).await?;
        let inode_id = InodeIdentifier {
            device_id: fs.device_id(),
            index: ROOT_INODE_INDEX,
        };
        fs_tree::mount_inode(parent_inode, inode_id);
        let fs: Arc<dyn FileSystem + Send> = fs;
        let mut vfs = lock_w_info!(VFS);
        vfs.mounted_filesystems.insert(part_id, fs);
    }

    Ok(())
}

async fn mount_new_root(fs: &Arc<dyn FileSystem + Send>) -> Result<(), ErrorCode> {
    // let inode = fs.stat(ROOT_INODE_INDEX).await?;
    // println!("Mounted root filesystem with root inode: {:X?}", inode);
    // let inode_index = inode.index;
    let inode_id = InodeIdentifier {
        device_id: fs.device_id(),
        index: ROOT_INODE_INDEX,
    };
    fs_tree::init(inode_id);

    //root checks
    let root_dirs = fs.read_dir(ROOT_INODE_INDEX).await?;
    let required_dirs = ["tty", "proc", "net"];
    #[allow(clippy::never_loop)]
    for required_dir in required_dirs.iter() {
        if !root_dirs.iter().any(|entry| entry.name.as_ref() == *required_dir) {
            println!("Root filesystem is missing required directory {required_dir}, creating it");
            //create the required directory
            fs.create(required_dir, ROOT_INODE_INDEX, InodeType::new_dir(0o755), 0, 0)
                .await?;
        }
    }
    Ok(())
}

async fn mount_vfs_adapters(mut vfs: NoIntSpinlockGuard<'_, Vfs>) {
    let proc_dev = VFS_ADAPTER_DEVICE.allocate_device(&mut vfs);
    let tty_dev = VFS_ADAPTER_DEVICE.allocate_device(&mut vfs);
    drop(vfs);

    let proc_adapter: Arc<dyn FileSystem + Send> = Arc::new(crate::vfs::adapters::ProcAdapter::new(proc_dev.0));
    let tty_adapter: Arc<dyn FileSystem + Send> = Arc::new(crate::vfs::adapters::TtyAdapter::new(tty_dev.0));
    Box::pin(mount_filesystem(resolve_path("/tty"), tty_adapter, tty_dev.1.partition))
        .await
        .expect("Failed to mount /tty");
    Box::pin(mount_filesystem(resolve_path("/proc"), proc_adapter, proc_dev.1.partition))
        .await
        .expect("Failed to mount /proc");
}

pub async fn unmount(path: ResolvedPathBorrowed<'_>) -> Result<(), ErrorCode> {
    let inodes = fs_tree::get_unmount_inodes(path, None).await?;
    let last_part_mount = fs_tree::unmount_inode(inodes.0);
    if last_part_mount {
        let mut vfs = lock_w_info!(VFS);
        let Some(device) = vfs.devices.get(&inodes.1.device_id) else {
            return Ok(());
        };
        let partition_id = device.partition;
        let Some(partition) = vfs.mounted_filesystems.remove(&partition_id) else {
            return Ok(());
        };
        partition.unmount().await?;
    }
    Ok(())
}

pub async fn open_file(
    path: ResolvedPathBorrowed<'_>,
    from: Option<InodeIdentifierChain>,
    mut open_mode: FileFlags,
) -> Result<FileHandle, ErrorCode> {
    let (inode_index, inode_chain) = fs_tree::get_inode_chain(path, from).await?;
    let open_file = get_file(inode_index).await?;
    open_mode.set_dir(unsafe { open_file.inode.get_read_ptr().type_mode.is_dir() });
    //TODO: check permissions
    Ok(FileHandle {
        inode: inode_index,
        parent_chain: inode_chain,
        position: 0,
        file_flags: open_mode,
        open_file,
    })
}

pub async fn get_dir_entries(file_handle: &FileHandle) -> Result<Box<[DirEntry]>, ErrorCode> {
    let inode = unsafe { file_handle.open_file.inode.get_read_ptr() };

    if !inode.type_mode.is_dir() {
        return Err(ErrorCode::UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.device).ok_or(ErrorCode::NoEntry)?;
    let partition_id = device_details.partition;
    let fs = vfs.mounted_filesystems.get_mut(&partition_id).ok_or(ErrorCode::NoEntry)?;
    let fs = fs.clone();
    drop(vfs);
    fs.read_dir(file_handle.inode.index).await
}

pub async fn create_file(parent_dir: &mut FileHandle, name: &str, inode_type: InodeType) -> Result<(), ErrorCode> {
    if !parent_dir.file_flags.write() {
        return Err(ErrorCode::InsufficientPermissions);
    }
    if !parent_dir.file_flags.dir() {
        return Err(ErrorCode::UnsupportedOperation);
    }

    let parent_inode = unsafe { parent_dir.open_file.inode.get_read_ptr() };
    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&parent_inode.device).ok_or(ErrorCode::InodeNotPresent)?;
    let partition_id = device_details.partition;
    let fs = vfs
        .mounted_filesystems
        .get_mut(&partition_id)
        .ok_or(ErrorCode::InodeNotPresent)?;
    let fs = fs.clone();
    drop(vfs);
    let (parent_inode, file_inode) = fs.create(name, parent_inode.index, inode_type, 0, 0).await?;
    println!(
        "create file returned file and parent inodes: {:X?}, {:X?}",
        file_inode, parent_inode
    );
    let child_id = InodeIdentifier {
        device_id: parent_inode.device,
        index: file_inode.index,
    };
    // fs_tree::update_inode(parent_dir.inode, parent_inode)?;
    fs_tree::insert_inode(parent_dir.inode, name.to_string().into_boxed_str(), child_id)?;
    parent_dir.open_file.inode.lock().await.update_from(&parent_inode);

    Ok(())
}

pub async fn write_file(file_handle: &mut FileHandle, buffer: &[PhysAddr], size: u64) -> Result<u64, ErrorCode> {
    // let inode = fs_tree::get_inode(file_handle.inode).ok_or(ErrorCode::InodeNotPresent)?;
    let mut inode = file_handle.open_file.inode.lock().await;

    let desired_offset = if file_handle.file_flags.append() {
        inode.size
    } else {
        file_handle.position
    };

    if !file_handle.file_flags.write() {
        return Err(ErrorCode::InsufficientPermissions);
    }

    if inode.type_mode.is_dir() {
        return Err(ErrorCode::UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.device).ok_or(ErrorCode::NoEntry)?;
    let partition_id = device_details.partition;
    let fs = vfs.mounted_filesystems.get_mut(&partition_id).ok_or(ErrorCode::NoEntry)?;
    let fs = fs.clone();
    drop(vfs);

    let res = fs.write(inode.index, desired_offset, size, buffer).await?;

    println!("operations::write_file: Wrote {} bytes", res.1);

    file_handle.position += res.1.min(size);

    inode.update_from(&res.0);

    Ok(res.1)
}

pub async fn stat_file(file_handle: &FileHandle) -> Inode {
    file_handle.open_file.inode.lock().await.clone()
}

pub async fn read_file(file_handle: &mut FileHandle, buffer: &[PhysAddr], size: u64) -> Result<u64, ErrorCode> {
    if !file_handle.file_flags.read() {
        return Err(ErrorCode::InsufficientPermissions);
    }

    let offset = file_handle.position;

    let inode = unsafe { file_handle.open_file.inode.get_read_ptr() };

    if inode.type_mode.is_dir() {
        return Err(ErrorCode::UnsupportedOperation);
    }

    let mut vfs = lock_w_info!(VFS);
    let device_details = vfs.devices.get(&inode.device).ok_or(ErrorCode::NoEntry)?;
    let partition_id = device_details.partition;
    let fs = vfs.mounted_filesystems.get_mut(&partition_id).ok_or(ErrorCode::NoEntry)?;
    let fs = fs.clone();
    drop(vfs);

    let bytes_read = fs.read(inode.index, offset, size, buffer).await?;

    println!("operations::read_file: Read {} bytes", bytes_read);

    let res = bytes_read.min(size);
    file_handle.position += res;
    Ok(res)
}
