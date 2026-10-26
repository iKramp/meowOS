use uuid::Uuid;

use crate::{
    drivers::{
        block_device::disk::DirEntry,
        input::{DeviceType, InputEvent},
    },
    memory::addresses::{PhysAddr, VirtAddr},
    vfs::{DeviceId as VfsDeviceId, FileReadResult, Inode, ROOT_INODE_INDEX, adapters::VfsAdapterTrait},
};
use std::{
    alloc::{
        self,
        boxed::Box,
        string::{String, ToString},
        vec::Vec,
    },
    error::KernelError,
    kerror, kerror_unwrapped, println,
    sync::{arc::Arc, once_lock::OnceLock},
};

#[derive(Debug)]
pub struct InputVfsAdapter {
    vfs_device_id: VfsDeviceId,
    device_details: crate::vfs::DeviceDetails,
}

impl InputVfsAdapter {
    pub fn get() -> Arc<dyn crate::vfs::FileSystem + Send> {
        static INPUT_ADAPTER: OnceLock<Arc<dyn crate::vfs::FileSystem + Send>> = OnceLock::new();
        INPUT_ADAPTER
            .get_or_init(|| {
                let device_details = crate::vfs::allocate_device();
                Arc::new(Self {
                    vfs_device_id: device_details.0,
                    device_details: device_details.1,
                })
            })
            .clone()
    }
}

const PREFIX_MASK: u64 = 0xF000_0000_0000_0000;
const STRUCTURE_FOLDER_PREFIX: u64 = 0x0000_0000_0000_0000;
const DEVICE_ID_PREFIX: u64 = 0x1000_0000_0000_0000;
const DEVICE_TYPE_PREFIX: u64 = 0x2000_0000_0000_0000;
const ALL_PREFIX: u64 = 0x3000_0000_0000_0000;

const INPUT_ROOT_INODE_INDEX: u64 = ROOT_INODE_INDEX | STRUCTURE_FOLDER_PREFIX;
const INPUT_DEVICE_TYPE_FOLDER_INODE_INDEX: u64 = (ROOT_INODE_INDEX + 1) | STRUCTURE_FOLDER_PREFIX;
const INPUT_DEVICE_ID_FOLDER_INODE_INDEX: u64 = (ROOT_INODE_INDEX + 2) | STRUCTURE_FOLDER_PREFIX;
const ALL_INPUT_INODE_INDEX: u64 = ALL_PREFIX;

fn get_buffer_id(extra_data: &[u8]) -> Result<u64, KernelError> {
    if extra_data.len() != 8 {
        return kerror!(InvalidArgument);
    }
    let mut buffer_id_bytes = [0u8; 8];
    buffer_id_bytes.copy_from_slice(&extra_data[..8]);
    Ok(u64::from_ne_bytes(buffer_id_bytes))
}

#[async_trait::async_trait]
impl VfsAdapterTrait for InputVfsAdapter {
    fn device_id(&self) -> VfsDeviceId {
        self.vfs_device_id
    }

    fn partition_id(&self) -> Uuid {
        self.device_details.partition
    }

    fn open(&self, inode: crate::vfs::InodeIndex) -> Result<Box<[u8]>, KernelError> {
        println!("open called with inode: {:#x}", inode);
        let inode_index = inode & !PREFIX_MASK;
        let prefix = inode & PREFIX_MASK;
        if prefix == STRUCTURE_FOLDER_PREFIX {
            // Open the structure folder
            return Ok(Box::new([]));
        }
        println!("open called with inode_index: {:#x}, prefix: {:#x}", inode_index, prefix);

        let (device_id, device_type) = if prefix == DEVICE_ID_PREFIX {
            (Some(inode_index), None)
        } else if prefix == DEVICE_TYPE_PREFIX {
            (None, Some(inode_index))
        } else if prefix == ALL_PREFIX {
            (None, None)
        } else {
            return kerror!(InvalidArgument);
        };
        let device_type = device_type.map(|t| DeviceType::from(t as u16));

        let buffer_id = super::INPUT_EVENT_MANAGER.add_buffer(device_type, device_id);

        println!("open called with inode: {:#x}, buffer_id: {}", inode, buffer_id);

        Ok(Box::new(buffer_id.to_ne_bytes()))
    }

    fn close(&self, inode: crate::vfs::InodeIndex, _extra_data: Box<[u8]>) -> Result<(), KernelError> {
        if inode & PREFIX_MASK == STRUCTURE_FOLDER_PREFIX {
            return Ok(());
        }
        let buffer_id = get_buffer_id(&_extra_data)?;
        super::INPUT_EVENT_MANAGER.remove_buffer(buffer_id);
        Ok(())
    }

    async fn read(
        &self,
        inode: crate::vfs::InodeIndex,
        extra_data: &Box<[u8]>,
        _offset_bytes: u64,
        mut size_bytes: u64,
        buffer: &[PhysAddr],
        mut blocking: bool,
    ) -> Result<(u64, FileReadResult), KernelError> {
        if inode & PREFIX_MASK == STRUCTURE_FOLDER_PREFIX {
            return kerror!(InvalidArgument);
        }

        if size_bytes == 0 {
            return Ok((0, FileReadResult::Normal));
        }
        if buffer.len() != size_bytes.div_ceil(4096) as usize {
            return kerror!(InvalidArgument);
        }

        let buffer_id = get_buffer_id(extra_data)?;

        let mut buffer_offset = 0;
        let mut buffer_index = 0;
        let mut total_elements_read = 0;
        loop {
            let buffer_virt = VirtAddr::from(buffer[buffer_index]) + buffer_offset;
            let buffer_ptr = buffer_virt.0 as *mut InputEvent;
            let buffer_len_bytes = (4096 - buffer_offset).min(size_bytes) as usize;
            let buffer_len_elements = buffer_len_bytes / std::mem::size_of::<InputEvent>();
            let buffer_slice = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, buffer_len_elements) };

            let elements_read = if blocking {
                super::INPUT_EVENT_MANAGER.consume_events_async(buffer_id, buffer_slice).await
            } else {
                super::INPUT_EVENT_MANAGER.consume_events(buffer_id, buffer_slice)
            };

            blocking = false; //only first read blocks
            total_elements_read += elements_read;
            let read_bytes = elements_read as u64 * std::mem::size_of::<InputEvent>() as u64;
            if elements_read != buffer_len_elements {
                return Ok((read_bytes, FileReadResult::TemporaryEof));
            }
            size_bytes -= read_bytes;

            if size_bytes == 0 {
                break;
            }

            //unaligned element
            let mut tmp_buffer = [InputEvent::default(); 1];
            let elements_read = super::INPUT_EVENT_MANAGER.consume_events(buffer_id, &mut tmp_buffer);
            if elements_read == 0 {
                break;
            }

            let write_1_ptr = buffer_virt.0 + read_bytes;
            let write_1_bytes = VirtAddr::from(buffer[buffer_index]).0 + 4096 - write_1_ptr;
            let write_1_ptr = write_1_ptr as *mut u8;
            let write_2_ptr = VirtAddr::from(buffer[buffer_index + 1]).0 as *mut u8;
            let write_2_bytes = core::mem::size_of::<InputEvent>() - write_1_bytes as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(tmp_buffer.as_ptr() as *const u8, write_1_ptr, write_1_bytes as usize);
                core::ptr::copy_nonoverlapping(tmp_buffer.as_ptr() as *const u8, write_2_ptr, write_2_bytes as usize);
            }
            buffer_offset = write_2_bytes as u64;
            buffer_index += 1;
            total_elements_read += 1;
        }

        println!(
            "read called with inode: {:#x}, buffer_id: {}, total_elements_read: {}",
            inode, buffer_id, total_elements_read
        );

        return Ok((
            total_elements_read as u64 * std::mem::size_of::<InputEvent>() as u64,
            FileReadResult::TemporaryEof,
        ));
    }

    async fn read_dir(&self, inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, KernelError> {
        if inode & PREFIX_MASK != STRUCTURE_FOLDER_PREFIX {
            return kerror!(InvalidArgument);
        }

        println!("read_dir called with inode: {:#x}", inode);

        let inode = inode & !PREFIX_MASK;

        match inode {
            INPUT_ROOT_INODE_INDEX => read_dir_root(),
            INPUT_DEVICE_TYPE_FOLDER_INODE_INDEX => read_dir_device_type(),
            INPUT_DEVICE_ID_FOLDER_INODE_INDEX => read_dir_device_id(),
            _ => kerror!(InvalidArgument),
        }
    }

    async fn write(
        &self,
        inode: crate::vfs::InodeIndex,
        extra_data: &Box<[u8]>,
        _offset: u64,
        mut size: u64,
        buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), KernelError> {
        if inode & PREFIX_MASK == STRUCTURE_FOLDER_PREFIX {
            return kerror!(InvalidArgument);
        }

        if size == 0 {
            return Ok((self.stat(inode).await?, 0));
        }
        if buffer.len() != size.div_ceil(4096) as usize {
            return kerror!(InvalidArgument);
        }

        let buffer_id = get_buffer_id(extra_data)?;

        let buffer_type = super::INPUT_EVENT_MANAGER
            .get_buffer_type(buffer_id)
            .ok_or(kerror_unwrapped!(InvalidArgument))?;
        //super::INPUT_EVENT_MANAGER.add_event(buffer_type.0, buffer_type.1, event);;

        let mut buffer_offset = 0;
        let mut buffer_index = 0;
        loop {
            let buffer_virt = VirtAddr::from(buffer[buffer_index]) + buffer_offset;
            let buffer_ptr = buffer_virt.0 as *const InputEvent;
            let buffer_len_bytes = (4096 - buffer_offset).min(size) as usize;
            let buffer_len_elements = buffer_len_bytes / std::mem::size_of::<InputEvent>();
            let buffer_slice = unsafe { core::slice::from_raw_parts(buffer_ptr, buffer_len_elements) };

            super::INPUT_EVENT_MANAGER.add_events(buffer_type.0, buffer_type.1, buffer_slice);

            let written_bytes = buffer_len_elements as u64 * std::mem::size_of::<InputEvent>() as u64;
            size -= written_bytes;

            if size == 0 {
                break;
            }

            //unaligned element
            let mut tmp_buffer = [InputEvent::default(); 1];
            let read_1_ptr = buffer_virt.0 + written_bytes;
            let read_1_bytes = VirtAddr::from(buffer[buffer_index]).0 + 4096 - read_1_ptr;
            let read_1_ptr = read_1_ptr as *const u8;
            let read_2_ptr = VirtAddr::from(buffer[buffer_index + 1]).0 as *const u8;
            let read_2_bytes = core::mem::size_of::<InputEvent>() - read_1_bytes as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(read_1_ptr, tmp_buffer.as_mut_ptr() as *mut u8, read_1_bytes as usize);
                core::ptr::copy_nonoverlapping(read_2_ptr, tmp_buffer.as_mut_ptr() as *mut u8, read_2_bytes as usize);
            }

            super::INPUT_EVENT_MANAGER.add_events(buffer_type.0, buffer_type.1, &tmp_buffer);
            buffer_offset = read_2_bytes as u64;
            buffer_index += 1;
        }

        Ok((self.stat(inode).await?, size))
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<Inode, KernelError> {
        Ok(Inode {
            index: inode,
            device: self.vfs_device_id,
            type_mode: if inode & PREFIX_MASK == STRUCTURE_FOLDER_PREFIX {
                crate::vfs::InodeTypeAndPerms::new_dir(0o755)
            } else {
                crate::vfs::InodeTypeAndPerms::new_file(0o777)
            },
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: 0,
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
            internal_synchronization: true,
        })
    }
}

fn read_dir_root() -> Result<Box<[DirEntry]>, KernelError> {
    Ok(alloc::vec![
        DirEntry {
            inode: INPUT_DEVICE_TYPE_FOLDER_INODE_INDEX,
            name: "device_type".to_string().into_boxed_str(),
        },
        DirEntry {
            inode: INPUT_DEVICE_ID_FOLDER_INODE_INDEX,
            name: "device_id".to_string().into_boxed_str(),
        },
        DirEntry {
            inode: ALL_INPUT_INODE_INDEX,
            name: "all".to_string().into_boxed_str(),
        },
    ]
    .into_boxed_slice())
}

fn read_dir_device_type() -> Result<Box<[DirEntry]>, KernelError> {
    let mut entries = Vec::new();
    for device_type in DeviceType::all_variants() {
        let (name, index): (String, u16) = device_type.into();
        entries.push(DirEntry {
            inode: (index as u64) | DEVICE_TYPE_PREFIX,
            name: name.into_boxed_str(),
        });
    }
    Ok(entries.into_boxed_slice())
}

fn read_dir_device_id() -> Result<Box<[DirEntry]>, KernelError> {
    Ok(Box::new([])) //empty for now, will have to query some dev subsystem
}
