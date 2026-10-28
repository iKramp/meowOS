use core::task::Waker;
use std::{
    alloc::{boxed::Box, vec::Vec},
    ffi_future, println,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
};

use crate::{
    drivers::block_device::disk::BlockDevice,
    memory::addresses::PhysAddr,
    task_runner::{self, PidOption},
    vfs::DeviceId,
};

const MAX_CACHE_ENTRIES_PER_DEVICE: usize = 4096;

pub(super) struct CacheEntry {
    pub data: Box<[u8; 512]>,
    dirty: bool,
    reading: bool,
}

pub(super) type CacheBlockEntry = (usize, CacheEntry);
pub(super) type CacheDeviceEntry = (DeviceId, Vec<CacheBlockEntry>);
pub(super) type EvictionCallback = Box<dyn FnOnce(Arc<dyn BlockDevice>) + Send + Sync>;

pub(super) struct BlockDeviceCache {
    cache: Vec<CacheDeviceEntry>,
    wait_callbacks: Vec<(DeviceId, usize, Waker)>,
}

pub(super) static BLOCK_DEVICE_CACHE: NoIntSpinlock<BlockDeviceCache> = NoIntSpinlock::new(BlockDeviceCache {
    cache: Vec::new(),
    wait_callbacks: Vec::new(),
});

pub(super) enum GetEntryResult<'a> {
    Found(&'a CacheEntry),
    Reading,
    NotFound,
}

impl BlockDeviceCache {
    //inserts a fake entry with reading if not found
    pub fn get_entry<'a>(&'a mut self, device_id: DeviceId, sector: usize, wait_callback: &Waker) -> GetEntryResult<'a> {
        let cache = &mut self.cache;
        let device_entry_pos = cache.iter().position(|(dev_id, _)| *dev_id == device_id);
        let device_entry_pos = match device_entry_pos {
            Some(entry) => entry,
            None => {
                let reading_entry = CacheEntry {
                    data: Box::new([0; 512]),
                    dirty: false,
                    reading: true,
                };
                let mut entries_vec = Vec::new();
                entries_vec.push((sector, reading_entry));
                cache.push((device_id, entries_vec));
                return GetEntryResult::NotFound;
            }
        };

        let sector_pos = cache[device_entry_pos].1.iter().position(|(sec, _)| *sec == sector);
        match sector_pos {
            Some(pos) => {
                let entry = &cache[device_entry_pos].1[pos].1;
                if entry.reading {
                    let wait_callbacks = &mut self.wait_callbacks;
                    wait_callbacks.push((device_id, sector, wait_callback.clone()));
                    GetEntryResult::Reading
                } else {
                    GetEntryResult::Found(entry)
                }
            }
            None => {
                let reading_entry = CacheEntry {
                    data: Box::new([0; 512]),
                    dirty: false,
                    reading: true,
                };
                cache[device_entry_pos].1.push((sector, reading_entry));
                GetEntryResult::NotFound
            }
        }
    }

    #[must_use]
    pub fn update_entry(
        &mut self,
        device_id: DeviceId,
        sector: usize,
        data: &[u8; 512],
        read_from_disk: bool,
    ) -> Option<EvictionCallback> {
        let device_entry_pos = self.cache.iter_mut().position(|(dev_id, _)| *dev_id == device_id);
        let device_entry_pos = match device_entry_pos {
            Some(entry) => entry,
            None => {
                self.cache.push((device_id, Vec::new()));
                self.cache.len() - 1
            }
        };
        let device_entry = &mut self.cache[device_entry_pos];
        let sector_entry_pos = device_entry.1.iter_mut().position(|(sec, _)| *sec == sector);
        match sector_entry_pos {
            Some(pos) => {
                let existing_entry = &mut device_entry.1[pos];
                if !existing_entry.1.reading && read_from_disk {
                    println!(level:warn, "outdated data from disk!");
                    return None;
                }
                existing_entry.1.reading = false;
                existing_entry.1.dirty = !read_from_disk;
                existing_entry.1.data.copy_from_slice(data);
                None
            }
            None => {
                let mut callback = None;
                if device_entry.1.len() >= MAX_CACHE_ENTRIES_PER_DEVICE {
                    callback = Self::evict_entry(&mut device_entry.1);
                }
                let mut entry = CacheEntry {
                    data: Box::new([0; 512]),
                    dirty: !read_from_disk,
                    reading: false,
                };
                entry.data.copy_from_slice(data);

                device_entry.1.push((sector, entry));
                callback
            }
        }
    }

    //returns if an entry was evicted. Can fail if all MAX entries are reading (unlikely)
    fn evict_entry(device_entry_vec: &mut Vec<CacheBlockEntry>) -> Option<EvictionCallback> {
        let clean = device_entry_vec.iter().position(|(_sec, entry)| !entry.dirty);
        if let Some(pos) = clean {
            device_entry_vec.swap_remove(pos);
            return None;
        }

        let pos = device_entry_vec.iter_mut().position(|(_sec, entry)| !entry.reading)?;
        let entry = device_entry_vec.swap_remove(pos);
        //is dirty
        let save_procedure = Box::new(move |disk: Arc<dyn BlockDevice>| {
            let fut = async move {
                disk.write(entry.0, 1, &[PhysAddr(entry.1.data.as_ptr() as u64)]).await;
            };
            let ffi_fut = ffi_future::future::into_ffi_future(fut);

            task_runner::add_task(ffi_fut, PidOption::None);
        });
        Some(save_procedure)
    }
}
