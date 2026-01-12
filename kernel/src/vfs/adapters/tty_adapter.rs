use std::error::ErrorCode;
use std::{lock_w_info, print};
use std::{sync::no_int_spinlock::NoIntSpinlock, vec::Vec};
use std::boxed::Box;

use crate::vfs::{DeviceId, InodeType};

use super::{VfsAdapterTrait, DirEntry};



#[derive(Debug)]
pub struct TtyAdapter { //for now whole /dev
    device_id: DeviceId,
    buffered_input: NoIntSpinlock<Vec<u8>>,
    ready_input: NoIntSpinlock<Vec<u8>>,
}

impl TtyAdapter {
    pub fn new(device_id: DeviceId) -> Self {
        TtyAdapter {
            device_id,
            buffered_input: NoIntSpinlock::new(Vec::new()),
            ready_input: NoIntSpinlock::new(Vec::new()),
        }
    }

    fn get_inode(&self) -> crate::vfs::Inode {
        crate::vfs::Inode {
            index: 0,
            device: self.device_id,
            type_mode: InodeType::new_file(0o777),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: lock_w_info!(self.ready_input).len() as u64,
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
    async fn read(&self, _inode: crate::vfs::InodeIndex, _offset_bytes: u64, mut size_bytes: u64, buffer: &[std::mem_utils::PhysAddr]) -> u64 {
        let mut ready_input = lock_w_info!(self.ready_input);
        let mut block = 0;
        let mut read_size = 0;
        loop {
            if size_bytes == 0 || ready_input.is_empty() {
                break;
            }
            let size_to_read = size_bytes.min(4096).min(ready_input.len() as u64);
            let Some(phys_ptr) = buffer.get(block as usize) else {
                break;
            };
            let ptr = std::mem_utils::translate_phys_virt_addr(*phys_ptr).0 as *mut u8;
            let slice = unsafe { core::slice::from_raw_parts_mut(ptr, size_to_read as usize) };
            slice.copy_from_slice(&ready_input[..size_to_read as usize]);
            ready_input.drain(..size_to_read as usize);
            block += 1;
            size_bytes -= size_to_read;
            read_size += size_to_read;
        }
        read_size
    }

    async fn read_dir(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, ErrorCode> {
        panic!("TTY does not support read_dir");
    }

    async fn write(&self, _inode: crate::vfs::InodeIndex, _offset: u64, size: u64, buffer: &[std::mem_utils::PhysAddr]) -> (crate::vfs::Inode, u64) {
        for i in 0..(size / 4096) {
            let Some(phys_ptr) = buffer.get(i as usize) else {
                return (self.get_inode(), size);
            };
            let ptr = std::mem_utils::translate_phys_virt_addr(*phys_ptr).0 as *const u8;
            let str = unsafe { core::str::from_raw_parts(ptr, 4096) };
            print!("{}", str);
        }
        let Some(phys_ptr) = buffer.last() else {
            return (self.get_inode(), size);
        };
        let ptr = std::mem_utils::translate_phys_virt_addr(*phys_ptr).0 as *const u8;
        let str = unsafe { core::str::from_raw_parts(ptr, (size % 4096) as usize) };
        print!("{}", str);

        (self.get_inode(), size)
    }

    async fn stat(&self, _inode: crate::vfs::InodeIndex) -> crate::vfs::Inode {
        self.get_inode()
    }
}
