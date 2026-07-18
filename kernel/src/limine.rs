#![allow(dead_code)]

use crate::memory::addresses::{PhysAddr, VirtAddr};

static LIMINE_BASE_REVISION: [u64; 3] = [0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 2];

pub static LIMINE_MEMMAP_USABLE: u64 = 0;
pub static LIMINE_MEMMAP_RESERVED: u64 = 1;
pub static LIMINE_MEMMAP_ACPI_RECLAIMABLE: u64 = 2;
pub static LIMINE_MEMMAP_ACPI_NVS: u64 = 3;
pub static LIMINE_MEMMAP_BAD_MEMORY: u64 = 4;
pub static LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE: u64 = 5;
pub static LIMINE_MEMMAP_KERNEL_AND_MODULES: u64 = 6;
pub static LIMINE_MEMMAP_FRAMEBUFFER: u64 = 7;

pub static mut LIMINE_BOOTLOADER_REQUESTS: BootloaderRequests = BootloaderRequests {
    _request_start_marker: [0xf6b8f4b39de7d1ae, 0xfab91a6940fcb9cf, 0x785c6ed015d3e316, 0x181e920a7852b9d9],
    bootloader_info_request: BootloaderInfoRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0xf55038d8e2a1202f, 0x279426fcf5f59740],
        revision: 0,
        info: core::ptr::null(),
    },
    cmd_line_request: CmdLineRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x4b161536e598651e, 0xb390ad4a2f1f303a],
        revision: 0,
        info: core::ptr::null(),
    },
    firmware_type_request: FirmwareTypeRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x8c2f75d90bef28a8, 0x7045a4688eac00],
        revision: 0,
        info: core::ptr::null(),
    },
    higher_half_direct_map_request: {
        HigherHalfDirectMapRequest {
            magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x48dcf1cb8ad2b852, 0x63984e959a98244b],
            revision: 0,
            info: core::ptr::null(),
        }
    },
    frame_buffer_request: FrameBufferRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x9d5827dcd881dd75, 0xa3148604f6fab11b],
        revision: 0,
        info: core::ptr::null(),
    },
    paging_mode_request: PagingModeRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x95c1a0edab0944cb, 0xa4e5cb3842f7488a],
        revision: 0,
        info: core::ptr::null(),
        mode: 0,
        min_mode: 0,
        max_mode: 0,
    },
    memory_map_request: MemoryMapRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x67cf3d9d378a806f, 0xe304acdfc50c3c62],
        revision: 0,
        info: core::ptr::null(),
    },
    rsdp_request: RsdpRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0xc5e77b6b397e7b43, 0x27637845accdcf3c],
        revision: 0,
        info: core::ptr::null(),
    },
    kernel_address_request: KernelAddressRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0x71ba76863cc55f63, 0xb2644a48c516a487],
        revision: 0,
        info: core::ptr::null(),
    },
    limine_kernel_file_request: LimineKernelFileRequest {
        magic: [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b, 0xad97e90e83f1ed67, 0x31eb5d1c5ff23b69],
        revision: 0,
        info: core::ptr::null(),
    },
    _request_end_marker: [0xadc0e0531bb10d03, 0x9572709f31764c62],
};

pub fn get_all() -> &'static mut BootloaderRequests {
    unsafe { &mut LIMINE_BOOTLOADER_REQUESTS }
}

#[repr(C)]
#[derive(Debug)]
pub struct BootloaderRequests {
    _request_start_marker: [u64; 4],

    pub bootloader_info_request: BootloaderInfoRequest,
    pub cmd_line_request: CmdLineRequest,
    pub firmware_type_request: FirmwareTypeRequest,
    pub higher_half_direct_map_request: HigherHalfDirectMapRequest,
    pub frame_buffer_request: FrameBufferRequest,
    pub paging_mode_request: PagingModeRequest,
    //pub smp_request: SMPRequest,
    pub memory_map_request: MemoryMapRequest,
    pub rsdp_request: RsdpRequest,
    pub kernel_address_request: KernelAddressRequest,
    pub limine_kernel_file_request: LimineKernelFileRequest,

    _request_end_marker: [u64; 2],
}

#[repr(C)]
#[derive(Debug)]
pub struct BootloaderInfoRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const BootloaderInfo,
}

#[repr(C)]
#[derive(Debug)]
pub struct BootloaderInfo {
    revision: u64,
    pub name: *const u8,
    pub version: *const u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct CmdLineRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const CmdLine,
}

#[repr(C)]
#[derive(Debug)]
pub struct CmdLine {
    revision: u64,
    pub cmdline: *const i8,
}

#[repr(C)]
#[derive(Debug)]
pub struct FirmwareTypeRequest {
    magic: [u64; 4],
    revision: u64,
    info: *const FirmwareType,
}

#[repr(C)]
#[derive(Debug)]
struct FirmwareType {
    revision: u64,
    firmware_type: u64, //0 = bios, 1 = uefi32, 2 = uefi64
}

#[repr(C)]
#[derive(Debug)]
pub struct HigherHalfDirectMapRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const HigherHalfDirectMap,
}

#[repr(C)]
#[derive(Debug)]
pub struct HigherHalfDirectMap {
    revision: u64,
    pub offset: u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct FrameBufferRequest {
    magic: [u64; 4],
    pub revision: u64,
    pub info: *const FrameBuffer,
}

#[repr(C)]
#[derive(Debug)]
pub struct FrameBuffer {
    pub revision: u64,
    pub framebuffer_count: u64,
    pub framebuffers: *const [&'static FramebufferInfo],
}

#[repr(C)]
#[derive(Debug)]
pub struct FramebufferInfo {
    pub address: *const (),
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
    pub unused: [u8; 7],
    pub edid_size: u64,
    pub edid: *const (),

    //v >= 1
    pub mode_count: u64,
    pub modes: *const [&'static FramebufferMode],
}

#[repr(C)]
#[derive(Debug)]
pub struct FramebufferMode {
    pub pitch: u64,
    pub width: u64,
    pub height: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct PagingModeRequest {
    magic: [u64; 4],
    revision: u64, //1
    info: *const u8,
    mode: u64,
    min_mode: u64, //set to 0
    max_mode: u64,
}

#[repr(C)]
#[derive(Debug)]
struct SMPRequest {
    //processors will be bootstrapped
    magic: [u64; 4],
    revision: u64,
    info: *const SMPInfo,
    flags: u64, //set to 0 to not enable x2apic, dealing with that is a pain
}

#[repr(C)]
#[derive(Debug)]
struct SMPInfo {
    revision: u64,
    flags: u32,
    bsp_lapic_id: u32,
    cpu_count: u64,
    cpu_info: *const [&'static CPUInfo],
}

#[repr(C)]
#[derive(Debug)]
struct CPUInfo {
    processor_id: u32,
    lapic_id: u32,
    reserved: u64,
    goto_address: PhysAddr,
    extra_argument: u64, //free to use
}

#[repr(C)]
#[derive(Debug)]
pub struct MemoryMapRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const MemoryMap,
}

#[repr(C)]
#[derive(Debug)]
pub struct MemoryMap {
    revision: u64,
    pub memory_map_count: u64,
    pub memory_map: *mut &'static mut MemoryMapEntry,
}

#[repr(C)]
#[derive(Debug)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub entry_type: u64,
}

impl MemoryMapEntry {
    pub fn is_usable(&self) -> bool {
        self.entry_type == LIMINE_MEMMAP_USABLE
    }

    pub fn can_be_usable(&self) -> bool {
        self.is_usable() || self.entry_type == LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct RsdpRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const Rsdp,
}

#[repr(C)]
#[derive(Debug)]
pub struct Rsdp {
    revision: u64,
    pub rsdp: *const (),
}

#[repr(C)]
#[derive(Debug)]
pub struct KernelAddressRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const KernelAddress,
}

#[repr(C)]
#[derive(Debug)]
pub struct KernelAddress {
    revision: u64,
    pub phys_addr: PhysAddr,
    pub virt_addr: VirtAddr,
}

#[repr(C)]
#[derive(Debug)]
pub struct Limine_UUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[repr(C)]
#[derive(Debug)]
pub struct LimineKernelFileRequest {
    magic: [u64; 4],
    revision: u64,
    pub info: *const LimineKernelFile,
}

#[repr(C)]
#[derive(Debug)]
pub struct LimineKernelFile {
    revision: u64,
    pub address: *const LimineFile,
}

#[repr(C)]
#[derive(Debug)]
pub struct LimineFile {
    pub revision: u64,
    pub address: *const (),
    pub size: u64,
    pub path: *const u8,
    pub cmdline: *const u8,
    pub media_type: u32,
    unused: u32,
    pub tftp_ip: u32,
    pub tftp_port: u32,
    pub partition_index: u32,
    pub mbr_disk_id: u32,
    pub gpt_disk_uuid: Limine_UUID,
    pub gpt_partition_uuid: Limine_UUID,
    pub part_uuid: Limine_UUID,
}
