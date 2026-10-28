use std::{
    boxed::Box,
    error::KernelError,
    kerror, println,
    sync::{arc::Arc, once_lock::OnceLock},
};

use uuid::Uuid;

use crate::{
    drivers::block_device::disk::DirEntry,
    memory::addresses::{PhysAddr, VirtAddr},
    vfs::{DeviceId, FileReadResult, FileSystem, Inode, adapters::VfsAdapterTrait},
    vga,
};

#[derive(Debug)]
pub struct FbAdapter {
    device_id: crate::vfs::DeviceId,
    device_details: crate::vfs::DeviceDetails,
}

static FB_ADAPTER: OnceLock<Arc<dyn FileSystem + Send>> = OnceLock::new();

impl FbAdapter {
    pub fn get() -> Arc<dyn FileSystem + Send> {
        FB_ADAPTER
            .get_or_init(|| {
                let device_details =
                    crate::vfs::VFS_ADAPTER_DEVICE.allocate_device(&mut crate::vfs::lock_w_info!(crate::vfs::VFS));
                Arc::new(Self {
                    device_id: device_details.0,
                    device_details: device_details.1,
                })
            })
            .clone()
    }

    fn get_inode(&self, index: crate::vfs::InodeIndex) -> crate::vfs::Inode {
        let dimensions = vga::vga_driver::get_dimensions();
        crate::vfs::Inode {
            index,
            device: self.device_id,
            type_mode: crate::vfs::InodeTypeAndPerms::new_file(0o777),
            link_cnt: 1,
            uid: 0,
            gid: 0,
            size: (dimensions.0 as u64) * (dimensions.1 as u64) * 3, // 3 bytes per pixel (RGB)
            access_time: 0,
            modification_time: 0,
            stat_change_time: 0,
            internal_synchronization: true,
            requires_aligned_position: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FrameBufferInfo {
    pub width: u32,
    pub height: u32,
}

#[async_trait::async_trait]
impl VfsAdapterTrait for FbAdapter {
    fn device_id(&self) -> DeviceId {
        self.device_id
    }

    fn partition_id(&self) -> Uuid {
        self.device_details.partition
    }

    fn open(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[u8]>, KernelError> {
        Ok(Box::new([]))
    }

    fn close(&self, _inode: crate::vfs::InodeIndex, _extra_data: Box<[u8]>) -> Result<(), KernelError> {
        Ok(())
    }

    async fn read(
        &self,
        _inode: crate::vfs::InodeIndex,
        _extra_data: &Box<[u8]>,
        _offset_bytes: u64,
        size_bytes: u64,
        buffer: &[PhysAddr],
        _blocking: bool,
    ) -> Result<(u64, FileReadResult), KernelError> {
        if size_bytes < core::mem::size_of::<FrameBufferInfo>() as u64 || buffer.is_empty() {
            return kerror!(InvalidArgument);
        }

        let dimensions = vga::vga_driver::get_dimensions();
        let framebuffer_info = FrameBufferInfo {
            width: dimensions.0 as u32,
            height: dimensions.1 as u32,
        };

        println!("FbAdapter::read: Writing framebuffer info {:?}", framebuffer_info);

        let dest_ptr = VirtAddr::from(buffer[0]).0 as *mut FrameBufferInfo;
        unsafe {
            *dest_ptr = framebuffer_info;
        }

        Ok((core::mem::size_of::<FrameBufferInfo>() as u64, FileReadResult::PermanentEof))
    }

    async fn read_dir(&self, _inode: crate::vfs::InodeIndex) -> Result<Box<[DirEntry]>, KernelError> {
        kerror!(UnsupportedOperation)
    }

    async fn write(
        &self,
        inode: crate::vfs::InodeIndex,
        _extra_data: &Box<[u8]>,
        mut fb_offset: u64,
        size: u64,
        buffer: &[PhysAddr],
    ) -> Result<(Inode, u64), KernelError> {
        let dimensions = vga::vga_driver::get_dimensions();
        let width = dimensions.0 as u32;
        let height = dimensions.1 as u32;
        let mut offset = 0_u64;

        fb_offset /= 3; // Each pixel consists of 3 bytes (RGB)

        if buffer.len() != size.div_ceil(4096) as usize {
            return kerror!(InvalidArgument);
        }

        for _ in 0..(size / 3) {
            let phys_addr_1 = *buffer.get((offset / 4096) as usize).unwrap_or(&buffer[0]) + (offset % 4096);
            offset += 1;
            let phys_addr_2 = *buffer.get((offset / 4096) as usize).unwrap_or(&buffer[0]) + (offset % 4096);
            offset += 1;
            let phys_addr_3 = *buffer.get((offset / 4096) as usize).unwrap_or(&buffer[0]) + (offset % 4096);
            offset += 1;

            let virt_addr_1 = VirtAddr::from(phys_addr_1).0 as *const u8;
            let virt_addr_2 = VirtAddr::from(phys_addr_2).0 as *const u8;
            let virt_addr_3 = VirtAddr::from(phys_addr_3).0 as *const u8;

            let color = unsafe { (*virt_addr_1, *virt_addr_2, *virt_addr_3) };

            let (x, y) = (fb_offset as usize % width as usize, fb_offset as usize / width as usize);

            if x >= width as usize || y >= height as usize {
                break;
            }

            vga::vga_driver::draw_pixel(x, y, color);

            fb_offset += 1;
        }

        Ok((self.get_inode(inode), size))
    }

    async fn stat(&self, inode: crate::vfs::InodeIndex) -> Result<Inode, KernelError> {
        Ok(self.get_inode(inode))
    }
}
