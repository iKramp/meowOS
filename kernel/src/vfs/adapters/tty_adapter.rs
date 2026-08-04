use std::boxed::Box;
use std::error::ErrorCode;
use std::sync::arc::Arc;
use std::sync::no_int_spinlock::NoIntSpinlock;
use std::sync::once_lock::OnceLock;
use std::{lock_w_info, println};

use uuid::Uuid;

use crate::memory::addresses::{PhysAddr, VirtAddr};
use crate::tty;
use crate::vfs::{DeviceId, FileSystem, Inode, InodeIndex, InodeTypeAndPerms};

use super::{DirEntry, VfsAdapterTrait};

static PROC_ADAPTER: OnceLock<Arc<dyn FileSystem + Send>> = OnceLock::new();

#[derive(Debug)]
pub struct TtyAdapter {
    device_id: crate::vfs::DeviceId,
    device_details: crate::vfs::DeviceDetails,
    write_lock: NoIntSpinlock<()>,
}

impl TtyAdapter {
    pub fn get() -> Arc<dyn FileSystem + Send> {
        PROC_ADAPTER
            .get_or_init(|| {
                let device_details = crate::vfs::VFS_ADAPTER_DEVICE.allocate_device(&mut *lock_w_info!(crate::vfs::VFS));
                println!("tty adapter created with device_id: {:?}", device_details.0);
                Arc::new(Self {
                    device_id: device_details.0,
                    device_details: device_details.1,
                    write_lock: NoIntSpinlock::new(()),
                })
            })
            .clone()
    }

    fn get_inode(&self, index: InodeIndex) -> crate::vfs::Inode {
        crate::vfs::Inode {
            index,
            device: self.device_id,
            type_mode: InodeTypeAndPerms::new_file(0o777),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: lock_w_info!(tty::TTY).data_len() as u64,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
        }
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for TtyAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    fn partition_id(&self) -> Uuid {
        self.device_details.partition
    }

    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset_bytes: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
    ) -> Result<u64, ErrorCode> {
        let Some(mut ready_input) = lock_w_info!(tty::TTY).get_input(size_bytes) else {
            return Ok(0);
        };
        let mut block = 0;
        let mut read_size = 0;
        loop {
            if ready_input.is_empty() {
                break;
            }
            let size_to_read = 4096.min(ready_input.len() as u64);
            let Some(phys_ptr) = buffer.get(block as usize) else {
                break;
            };
            let virt_ptr: VirtAddr = (*phys_ptr).into();
            let ptr = virt_ptr.0 as *mut u8;
            let slice = unsafe { core::slice::from_raw_parts_mut(ptr, size_to_read as usize) };
            slice.copy_from_slice(&ready_input.as_bytes()[..size_to_read as usize]);
            ready_input.drain(..size_to_read as usize);
            block += 1;
            read_size += size_to_read;
        }
        Ok(read_size)
    }

    async fn read_dir(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode> {
        Err(ErrorCode::UnsupportedOperation)
    }

    async fn write(
        &self,
        inode: crate::vfs::InodeIndex,
        _offset: u64,
        size: u64,
        buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), ErrorCode> {
        let tty = lock_w_info!(tty::TTY);
        for i in 0..(size / 4096) {
            let Some(phys_ptr) = buffer.get(i as usize) else {
                return Err(ErrorCode::InvalidArgument);
            };
            let virt_ptr: VirtAddr = (*phys_ptr).into();
            let ptr = virt_ptr.0 as *const u8;
            let str = unsafe { core::str::from_raw_parts(ptr, 4096) };
            tty.print(str);
        }
        let Some(phys_ptr) = buffer.last() else {
            return Ok((self.get_inode(inode), size));
        };
        let virt_ptr: VirtAddr = (*phys_ptr).into();
        let ptr = virt_ptr.0 as *const u8;
        let str = unsafe { core::str::from_raw_parts(ptr, (size % 4096) as usize) };

        tty.print(str);

        drop(tty);

        Ok((self.get_inode(inode), size))
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<Inode, ErrorCode> {
        Ok(self.get_inode(inode))
    }
}
