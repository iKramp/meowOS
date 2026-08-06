use core::ptr::addr_of;
use core::sync::atomic::AtomicU64;
use std::collections::btree_map::BTreeMap;
use std::fmt::Debug;
use std::string::ToString;
use std::sync::arc::Arc;
use std::sync::once_lock::OnceLock;
use std::sync::rw_lock::{RWLockModeRead, RWSpinlock, RWSpinlockGuard};
use std::{boxed::Box, error::KernelError, vec::Vec};
use std::{kerror, kerror_unwrapped, lock_w_info, println, r_lock_w_info, w_lock_w_info};

use crate::drivers::block_device::disk::DirEntry;
use crate::memory::addresses::*;
use crate::vfs::adapters::VfsAdapterTrait;
use crate::vfs::{DeviceId, FileSystem, Inode, InodeIndex, InodeTypeAndPerms, inode};

const ENTRIES_PER_NIC_DEVICE: u64 = 2;
const _: () = assert!(ENTRIES_PER_NIC_DEVICE.is_power_of_two());

static NET_ADAPTER: OnceLock<Arc<dyn FileSystem + Send>> = OnceLock::new();

enum NICEntryType {
    MainFolder = 0,
    MacAddress = 1,
    Data = 2,
    Mtu = 3,
}

pub trait NicAdapter: Send + Sync + Debug {
    fn get_mac_address(&self) -> [u8; 6];
    fn send_packet(&self, data: &[u8]) -> Result<(), KernelError>;
    fn mtu(&self) -> usize;
}

type NetAdapterEtherDeviceMap = BTreeMap<InodeIndex, Box<dyn NicAdapter>>;

#[derive(Debug)]
pub struct NetAdapter {
    ether_devices: RWSpinlock<NetAdapterEtherDeviceMap>,
    inode_counter: AtomicU64,
    device_id: crate::vfs::DeviceId,
    device_details: crate::vfs::DeviceDetails,
}

impl NetAdapter {
    pub fn get() -> Arc<dyn FileSystem + Send> {
        NET_ADAPTER
            .get_or_init(|| {
                let device_details = crate::vfs::VFS_ADAPTER_DEVICE.allocate_device(&mut lock_w_info!(crate::vfs::VFS));
                println!("proc adapter created with device_id: {:?}", device_details.0);
                Arc::new(Self {
                    device_id: device_details.0,
                    device_details: device_details.1,
                    ether_devices: RWSpinlock::new(BTreeMap::new()),
                    inode_counter: AtomicU64::new(ENTRIES_PER_NIC_DEVICE),
                })
            })
            .clone()
    }

    pub fn register_ether_device(&mut self, device: Box<dyn NicAdapter>) {
        let inode_index = self
            .inode_counter
            .fetch_add(ENTRIES_PER_NIC_DEVICE, core::sync::atomic::Ordering::SeqCst);
        w_lock_w_info!(self.ether_devices).insert(inode_index, device);
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for NetAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    fn partition_id(&self) -> uuid::Uuid {
        self.device_details.partition
    }

    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset_bytes: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
    ) -> Result<u64, KernelError> {
        if size_bytes == 0 {
            return Ok(0);
        }
        if buffer.len() != size_bytes.div_ceil(4096) as usize {
            return kerror!(InvalidArgument);
        }

        let devices = r_lock_w_info!(self.ether_devices);
        let (device, entry_type) = match get_ether_device_from_inode(&devices, _inode) {
            Some(v) => v,
            None => return kerror!(InodeNotPresent),
        };

        match entry_type {
            NICEntryType::MacAddress => {
                let mac_address = device.get_mac_address();
                let read_size = size_bytes.min(6);
                let first_buffer = buffer[0];
                let first_buffer_virt: VirtAddr = first_buffer.into();
                let ptr = first_buffer_virt.0 as *mut u8;
                unsafe { core::ptr::copy_nonoverlapping(addr_of!(mac_address) as *const u8, ptr, read_size as usize) };
                Ok(read_size)
            }
            NICEntryType::Mtu => {
                let mtu = device.mtu() as u32;
                let read_size = size_bytes.min(4);
                let first_buffer = buffer[0];
                let first_buffer_virt: VirtAddr = first_buffer.into();
                let ptr = first_buffer_virt.0 as *mut u8;
                unsafe { core::ptr::copy_nonoverlapping(addr_of!(mtu) as *const u8, ptr, read_size as usize) };
                Ok(read_size)
            }
            _ => return kerror!(UnsupportedOperation),
        }
    }

    async fn read_dir(&self, inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, KernelError> {
        let entry_type = get_entry_type_from_inode(inode).ok_or(kerror_unwrapped!(InodeNotPresent))?;

        return match entry_type {
            NICEntryType::MainFolder => {
                let mut entries = Vec::new();
                let base_inode = inode - (inode % ENTRIES_PER_NIC_DEVICE);
                entries.push(DirEntry {
                    inode: 0,
                    name: "..".to_string().into_boxed_str(),
                });
                entries.push(DirEntry {
                    inode: base_inode,
                    name: ".".to_string().into_boxed_str(),
                });
                entries.push(DirEntry {
                    inode: base_inode + 1,
                    name: "mac_address".to_string().into_boxed_str(),
                });
                entries.push(DirEntry {
                    inode: base_inode + 2,
                    name: "data".to_string().into_boxed_str(),
                });
                entries.push(DirEntry {
                    inode: base_inode + 3,
                    name: "mtu".to_string().into_boxed_str(),
                });
                Ok(entries.into_boxed_slice())
            }
            _ => kerror!(UnsupportedOperation),
        };
    }

    async fn write(&self, inode: InodeIndex, _offset: u64, size: u64, buffer: &[PhysAddr]) -> Result<(Inode, u64), KernelError> {
        if size == 0 {
            return Ok((VfsAdapterTrait::stat(self, inode).await?, 0));
        }
        if buffer.len() != size.div_ceil(4096) as usize {
            return kerror!(InvalidArgument);
        }

        let devices = r_lock_w_info!(self.ether_devices);
        let (device, entry_type) = match get_ether_device_from_inode(&devices, inode) {
            Some(v) => v,
            None => return kerror!(InodeNotPresent),
        };

        match entry_type {
            NICEntryType::Data => {
                let mtu = device.mtu();
                if size as usize > mtu {
                    return kerror!(InvalidArgument);
                }

                if size > 4096 {
                    //for now
                    return kerror!(InvalidArgument);
                }

                let virt: VirtAddr = buffer[0].into();
                let ptr = virt.0 as *const u8;
                let slice = unsafe { core::slice::from_raw_parts(ptr, size as usize) };
                device.send_packet(slice)?;

                Ok((VfsAdapterTrait::stat(self, inode).await?, size))
            }
            _ => return kerror!(UnsupportedOperation),
        }
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<crate::vfs::Inode, KernelError> {
        let entry_type = match get_entry_type_from_inode(inode) {
            Some(v) => v,
            None => return kerror!(InodeNotPresent),
        };
        let stat = match entry_type {
            NICEntryType::MainFolder => inode::Inode {
                index: inode,
                device: self.device_id,
                type_mode: InodeTypeAndPerms::new_dir(0o444),
                link_cnt: 1,
                uid: 0,
                gid: 0,
                size: 0,
                access_time: 0,
                modification_time: 0,
                stat_change_time: 0,
            },
            NICEntryType::MacAddress | NICEntryType::Mtu => inode::Inode {
                index: inode,
                device: self.device_id,
                type_mode: InodeTypeAndPerms::new_file(0o444),
                link_cnt: 1,
                uid: 0,
                gid: 0,
                size: 6,
                access_time: 0,
                modification_time: 0,
                stat_change_time: 0,
            },
            NICEntryType::Data => inode::Inode {
                index: inode,
                device: self.device_id,
                type_mode: InodeTypeAndPerms::new_file(0o600),
                link_cnt: 1,
                uid: 0,
                gid: 0,
                size: 0,
                access_time: 0,
                modification_time: 0,
                stat_change_time: 0,
            },
        };
        Ok(stat)
    }
}

fn get_ether_device_from_inode<'a>(
    devices: &'a RWSpinlockGuard<NetAdapterEtherDeviceMap, RWLockModeRead>,
    inode: InodeIndex,
) -> Option<(&'a dyn NicAdapter, NICEntryType)> {
    let device_index = inode - (inode % ENTRIES_PER_NIC_DEVICE);
    let device = devices.get(&device_index)?.as_ref();
    let entry_type = get_entry_type_from_inode(inode)?;
    Some((device, entry_type))
}

fn get_entry_type_from_inode(inode: InodeIndex) -> Option<NICEntryType> {
    match inode % ENTRIES_PER_NIC_DEVICE {
        0 => Some(NICEntryType::MainFolder),
        1 => Some(NICEntryType::MacAddress),
        2 => Some(NICEntryType::Data),
        3 => Some(NICEntryType::Mtu),
        _ => None,
    }
}
