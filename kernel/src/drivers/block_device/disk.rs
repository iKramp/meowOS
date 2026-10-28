use crate::{
    drivers::block_device::block_dev_cache::{BLOCK_DEVICE_CACHE, GetEntryResult},
    memory::{addresses::*, physical_allocator},
};
use core::{
    cell::{Cell, UnsafeCell},
    fmt::Debug,
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};
use std::{alloc::boxed::Box, lock_w_info, string::String, sync::arc::Arc, vec::Vec};

use uuid::Uuid;

use crate::vfs::{DeviceId, InodeIndex};

//assuming 512B sector
const SECTORS_PER_PAGE: usize = 4096 / 512;

const USE_CACHE: bool = true;

#[async_trait::async_trait]
pub trait BlockDevice: Debug + Send + Sync {
    async fn read(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]);
    async fn write(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]);
}

#[async_trait::async_trait]
pub trait PartitionSchemeDriver {
    async fn guid(&self, disk: &dyn BlockDevice) -> Uuid;
    ///returns a vector of partition guids (not filesystem ids) and partition objects
    async fn partitions(&self, disk: &dyn BlockDevice) -> Vec<(Uuid, Partition)>;
}

//Make sure this is ALWAYS send + sync
#[derive(Debug)]
pub struct MountedPartition {
    pub disk: Arc<dyn BlockDevice>,
    pub partition: Partition,
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub fs_type: Uuid,
    pub part_id: Uuid,
    pub device: DeviceId,
    pub start_sector: usize,
    pub size_sectors: usize,
    pub name: String,
}

impl MountedPartition {
    pub fn new(disk: Arc<dyn BlockDevice>, partition: Partition) -> Self {
        Self { disk, partition }
    }

    pub async fn read(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]) {
        assert!(sector + sec_count <= self.partition.size_sectors);
        if USE_CACHE {
            CacheReader {
                device: self.disk.clone(),
                device_ref: self.disk.get(),
                device_id: self.partition.device,
                sector,
                sec_count,
                read_sectors: Cell::new(0),
                currently_reading_addr: ManuallyDrop::new(UnsafeCell::new(OwnedPhysAddr(PhysAddr(0)))),
                currently_reading_fut: UnsafeCell::new(None),
                destination: buffer,
            }
            .await;
        } else {
            self.disk.read(self.partition.start_sector + sector, sec_count, buffer).await;
        }
    }

    pub async fn write(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]) {
        assert!(sector + sec_count <= self.partition.size_sectors);
        if USE_CACHE {
            let mut cache = lock_w_info!(BLOCK_DEVICE_CACHE);
            for i in 0..sec_count {
                let arr_index = i / SECTORS_PER_PAGE;
                let src_ptr = VirtAddr::from(buffer[arr_index]).0 as *const u8;
                let slice = unsafe { core::slice::from_raw_parts(src_ptr, 4096) };
                let chunks: &[[u8; 512]] = slice.as_chunks().0;
                let data = chunks[i % SECTORS_PER_PAGE];
                let callback = cache.update_entry(self.partition.device, sector, &data, false);
                if let Some(callback) = callback {
                    callback(self.disk.clone());
                }
            }
        } else {
            self.disk.write(self.partition.start_sector + sector, sec_count, buffer).await;
        }
    }
}

#[derive(Debug)]
pub struct DirEntry {
    pub inode: InodeIndex,
    pub name: Box<str>,
}

pub struct CacheReader<'a> {
    device: Arc<dyn BlockDevice>,
    device_ref: &'a dyn BlockDevice,
    device_id: DeviceId,
    sector: usize,
    sec_count: usize,
    read_sectors: Cell<usize>,
    currently_reading_addr: ManuallyDrop<UnsafeCell<OwnedPhysAddr>>,
    currently_reading_fut: UnsafeCell<Option<Pin<Box<dyn Future<Output = ()> + 'a + Send>>>>,
    destination: &'a [PhysAddr],
}

impl<'a> Future for CacheReader<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            'check_curr_fut: {
                let mut curr_fut = None;
                unsafe { core::ptr::swap(&raw mut curr_fut, self.currently_reading_fut.get()) };

                let Some(mut curr_fut) = curr_fut else {
                    break 'check_curr_fut;
                };
                let res = Future::poll(curr_fut.as_mut(), cx);
                match res {
                    Poll::Ready(()) => {
                        //read SECTORS_PER_PAGE sectors into currently_reading_addr
                        let phys_addr = unsafe { *(self.currently_reading_addr.get() as *const PhysAddr) };

                        let mut cache = lock_w_info!(BLOCK_DEVICE_CACHE);
                        let page_ptr = VirtAddr::from(phys_addr).0 as *const u8;
                        let page_slice = unsafe { core::slice::from_raw_parts(page_ptr, 4096) };
                        let chunks: &[[u8; 512]] = page_slice.as_chunks().0;

                        let read_sectors = self.read_sectors.get();
                        let curr_sector = self.sector + read_sectors;

                        for (i, sec_data) in chunks.iter().enumerate() {
                            let update_res = cache.update_entry(self.device_id, curr_sector + i, sec_data, true);
                            if let Some(callback) = update_res {
                                callback(self.device.clone())
                            }

                            if read_sectors >= self.sec_count {
                                continue;
                            }

                            let dest_arr_index = (read_sectors + i) / SECTORS_PER_PAGE;
                            let offset_in_page = ((read_sectors + i) % SECTORS_PER_PAGE) * 512;
                            let dest = VirtAddr::from(self.destination[dest_arr_index]).0 + offset_in_page as u64;
                            let dest = dest as *mut u8;
                            let src = sec_data.as_ptr();
                            unsafe { core::ptr::copy(src, dest, 512) };
                        }
                        self.read_sectors.set(read_sectors + chunks.len());

                        //last
                        let owned_addr = OwnedPhysAddr(phys_addr);
                        drop(owned_addr);
                    }
                    Poll::Pending => {
                        unsafe { *self.currently_reading_fut.get() = Some(curr_fut) }
                        return Poll::Pending;
                    }
                }
            }

            let read_sectors = self.read_sectors.get();
            let to_read_sector = self.sector + read_sectors;

            if read_sectors >= self.sec_count {
                return Poll::Ready(());
            }

            let mut cache = lock_w_info!(BLOCK_DEVICE_CACHE);

            let entry = cache.get_entry(self.device_id, to_read_sector, cx.waker());
            match entry {
                GetEntryResult::Found(cache_entry) => {
                    let dest_arr_index = read_sectors / SECTORS_PER_PAGE;
                    let offset_in_page = (read_sectors % SECTORS_PER_PAGE) * 512;
                    let dest = VirtAddr::from(self.destination[dest_arr_index]).0 + offset_in_page as u64;
                    let dest = dest as *mut u8;

                    let src = cache_entry.data.as_ptr();
                    unsafe { core::ptr::copy(src, dest, 512) };
                    self.read_sectors.set(read_sectors + 1);
                }
                GetEntryResult::Reading => return Poll::Pending, //will be waken up eventually
                GetEntryResult::NotFound => {
                    let frame = physical_allocator::allocate();
                    unsafe { *self.currently_reading_addr.get() = frame };

                    let phys_addr_ptr = self.currently_reading_addr.get();
                    let phys_addr_slice = unsafe { core::slice::from_raw_parts(phys_addr_ptr as *const PhysAddr, 1) };

                    let res = Box::pin(self.device_ref.read(to_read_sector, SECTORS_PER_PAGE, phys_addr_slice));
                    unsafe { *self.currently_reading_fut.get() = Some(res) }
                    continue;
                }
            }
        }
    }
}
