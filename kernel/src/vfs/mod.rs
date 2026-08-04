use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    lock_w_info, println,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock, rw_lock::RWSpinlock},
    vec::Vec,
};
use uuid::Uuid;

use crate::drivers::block_device::disk::{BlockDevice, Partition};

mod adapters;
mod dtmpfs;
pub mod file;
mod filesystem_trait;
mod fs_syscall_pack;
mod fs_tree;
mod inode;
mod operations;
mod path;
pub use filesystem_trait::{FileSystem, FileSystemFactory};
pub use inode::*;
pub use operations::*;
pub use path::*;

//0 is unknown, 1 is bad blocks, 2 is root
pub const ROOT_INODE_INDEX: u64 = 2;
pub static VFS: NoIntSpinlock<Vfs> = NoIntSpinlock::new(Vfs::new());
static VFS_ADAPTER_DEVICE: adapters::VfsAdapterDevice = adapters::VfsAdapterDevice::new();
///maps from mount point inode to mounted inode. Arc because it's shared across async so a lock to VFS can't be held
static GLOBAL_MOUNTS: RWSpinlock<BTreeMap<InodeIdentifier, InodeIdentifier>> = RWSpinlock::new(BTreeMap::new());

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[rustc_layout_scalar_valid_range_end(0xFFFF_FFFF_FFFF_FFFE)]
pub struct DeviceId(u64);

impl DeviceId {
    pub const fn new(id: u64) -> Self {
        unsafe { DeviceId(id) }
    }
}

pub type InodeIndex = u64;

#[derive(Debug, Clone)]
pub struct DeviceDetails {
    pub drive: Uuid,
    pub partition: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InodeIdentifier {
    pub device_id: DeviceId,
    pub index: InodeIndex,
}

///Always at least 1 element - root
pub type InodeIdentifierChain = Box<[InodeIdentifier]>;

pub struct Vfs {
    ///Map from disk guid to disk object (driver) and a list of partition guids
    disks: BTreeMap<Uuid, (Arc<dyn BlockDevice + Send>, Vec<Uuid>)>,
    ///maps from filesystem type uuid to filesystem driver factory
    filesystem_driver_factories: BTreeMap<Uuid, Arc<dyn FileSystemFactory + Send>>,
    ///maps from partition uuid to filesystem driver
    mounted_filesystems: BTreeMap<Uuid, Arc<dyn FileSystem + Send>>,
    ///maps from partition uuid to partition object
    available_partitions: BTreeMap<Uuid, Partition>,
    ///maps from device id to partition and drive uuid
    devices: BTreeMap<DeviceId, DeviceDetails>,
    ///counts devices
    device_counter: u64,
}

impl Vfs {
    const fn new() -> Self {
        Vfs {
            disks: BTreeMap::new(),
            filesystem_driver_factories: BTreeMap::new(),
            mounted_filesystems: BTreeMap::new(),
            available_partitions: BTreeMap::new(),
            devices: BTreeMap::new(),
            device_counter: 1,
        }
    }

    pub fn allocate_device(&mut self) -> DeviceId {
        let id = self.device_counter;
        self.device_counter += 1;
        DeviceId::new(id)
    }

    pub fn print_available_fs_driver_types(&self) {
        println!(level:debug, "Available Fs types: ");
        for part in self.filesystem_driver_factories.iter() {
            println!(level:debug, "{} ({})", part.0, part.1.name());
        }
    }
}

pub fn init() {
    dtmpfs::init_dtmpfs();

    fs_syscall_pack::init_fs_syscall_pack();
}

pub fn register_filesystem_driver_factory(factory: Arc<dyn FileSystemFactory + Send>) {
    let mut vfs = lock_w_info!(VFS);
    vfs.filesystem_driver_factories.insert(factory.uuid(), factory.clone());
}

pub fn print_available_fs_driver_types() {
    let vfs = lock_w_info!(VFS);
    vfs.print_available_fs_driver_types();
}
