use std::boxed::Box;
use std::error::ErrorCode;
use std::sync::no_int_spinlock::NoIntSpinlock;
use std::{Print, lock_w_info};

use crate::tty;
use crate::vfs::{DeviceId, Inode, InodeIndex, InodeType};
use crate::vga::vga_text;

use super::{DirEntry, VfsAdapterTrait};

#[derive(Debug)]
pub struct TtyAdapter {
    device_id: DeviceId,
    write_lock: NoIntSpinlock<()>,
}

impl TtyAdapter {
    pub fn new(device_id: DeviceId) -> Self {
        TtyAdapter {
            device_id,
            write_lock: NoIntSpinlock::new(()),
        }
    }

    fn get_inode(&self, index: InodeIndex) -> crate::vfs::Inode {
        crate::vfs::Inode {
            index,
            device: self.device_id,
            type_mode: InodeType::new_file(0o777),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: lock_w_info!(tty::TTY).data_len() as u64,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
            preferred_block_size: 512,
            blocks: u32::MAX,
        }
    }
}

#[async_trait::async_trait]
impl VfsAdapterTrait for TtyAdapter {
    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _offset_bytes: u64,
        size_bytes: u64,
        buffer: &[std::mem_utils::PhysAddr],
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
            let ptr = std::mem_utils::translate_phys_virt_addr(*phys_ptr).0 as *mut u8;
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
        buffer: &[std::mem_utils::PhysAddr],
    ) -> Result<(Inode, u64), ErrorCode> {
        let tty = lock_w_info!(tty::TTY);
        for i in 0..(size / 4096) {
            let Some(phys_ptr) = buffer.get(i as usize) else {
                return Err(ErrorCode::InvalidArgument);
            };
            let ptr = std::mem_utils::translate_phys_virt_addr(*phys_ptr).0 as *const u8;
            let str = unsafe { core::str::from_raw_parts(ptr, 4096) };
            tty.print(str);
        }
        let Some(phys_ptr) = buffer.last() else {
            return Ok((self.get_inode(inode), size));
        };
        let ptr = std::mem_utils::translate_phys_virt_addr(*phys_ptr).0 as *const u8;
        let str = unsafe { core::str::from_raw_parts(ptr, (size % 4096) as usize) };

        tty.print(str);

        drop(tty);

        Ok((self.get_inode(inode), size))
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<Inode, ErrorCode> {
        Ok(self.get_inode(inode))
    }
}
