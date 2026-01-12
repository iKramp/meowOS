use core::fmt::Debug;

use bitfield::bitfield;

#[derive(Debug)]
pub enum FisType {
    RegisterH2D = 0x27,
    RegisterD2H = 0x34,
    DMAActivate = 0x39,
    DMASetup = 0x41,
    Data = 0x46,
    Bist = 0x58,
    PIOSetup = 0x5F,
    SetDeviceBits = 0xA1,
}

#[repr(C)]
struct DataFis {
    fis_type: FisType,
    port_multiplier: u8,
    reserved: [u8; 2],
    data: [u8],
}

#[derive(Debug)]
#[repr(C)]
pub struct H2DRegisterFis {
    pub fis_type: u8,
    ///set to 1 for command, 0 for control
    pub pmport: H2DRegFisPmport,
    pub command: u8,
    pub featurel: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub featureh: u8,

    pub countl: u8,
    pub counth: u8,
    pub icc: u8,
    pub control: u8,

    pub reserved: [u8; 4],
}

impl From<&H2DRegisterFis> for &[u8] {
    fn from(fis: &H2DRegisterFis) -> Self {
        unsafe {
            core::slice::from_raw_parts(
                fis as *const H2DRegisterFis as *const u8,
                core::mem::size_of::<H2DRegisterFis>(),
            )
        }
    }
}

impl Default for H2DRegisterFis {
    fn default() -> Self {
        H2DRegisterFis {
            fis_type: FisType::RegisterH2D as u8,
            pmport: H2DRegFisPmport(0),
            command: 0,
            featurel: 0,
            lba0: 0,
            lba1: 0,
            lba2: 0,
            device: 0,
            lba3: 0,
            lba4: 0,
            lba5: 0,
            featureh: 0,
            countl: 0,
            counth: 0,
            icc: 0,
            control: 0,
            reserved: [0; 4],
        }
    }
}

bitfield! {
    pub struct H2DRegFisPmport(u8);
    impl Debug;
    pub pmport, set_pmport: 3, 0;
    //reserved 6, 3
    pub command, set_command: 7;
}

#[derive(Debug)]
#[repr(C)]
pub struct D2HRegisterFis {
    pub fis_type: u8,
    pub pmport: u8,
    pub status: u8,
    pub error: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    reserved: u8,

    pub countl: u8,
    pub counth: u8,
    reserved2: [u8; 2],

    reserved3: [u8; 4],
}

#[derive(Debug)]
#[repr(C)]
pub struct PioSetupFis {
    pub fis_type: u8,
    pub pmport: u8,
    pub status: u8,
    pub error: u8,

    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,

    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub reserved: u8,

    pub countl: u8,
    pub counth: u8,
    pub reserved2: u8,
    pub e_status: u8,

    pub transfer_count: u16,
    pub reserved3: [u8; 2],
}

#[derive(Debug)]
#[repr(C)]
pub struct DmaSetupFis {
    pub fis_type: u8,
    pub pmport: DmaPmport,
    pub reserved: [u8; 2],
    pub buffer_id: u64,
    pub reserved2: [u8; 4],
    pub byte_count: u32,
    pub reserved3: [u8; 4],
}

#[derive(Debug)]
#[repr(C)]
pub struct SetDeviceBits {
    pub fis_type: u8,
    pub flags: u8,
    pub reserved: [u8; 2],
    pub control: u32,
}

bitfield! {
    pub struct SetDeviceBitsFlags(u8);
    impl Debug;

}

bitfield! {
    pub struct DmaPmport(u8);
    impl Debug;
    pmport, set_pmport: 3, 0;
    dir, set_dir: 5;
    interrupt, set_interrupt: 6;
    auto_activate, set_auto_activate: 7;
}

//not really a fis, it's a structure returned by IDENTIFY command

#[repr(C)]
pub struct IdentifyStructure {
    pub general_config: u16,
    pub cyls: u16,
    pub reserved2: u16,
    pub heads: u16,
    pub track_bytes: u16,
    pub sector_bytes: u16,
    pub sectors: u16,
    pub vendor0: u16,
    pub vendor1: u16,
    pub vendor2: u16,
    pub serial_no: [u8; 20],
    pub buf_type: u16,
    pub buf_size: u16,
    pub ecc_bytes: u16,
    pub fw_rev: [u8; 8],
    pub model: [u8; 40],
    pub multi_count: u16,
    pub dword_io: u16,
    pub capability1: u16,
    pub capability2: u16,
    pub vendor5: u8,
    pub tpio: u8,
    pub vendor6: u8,
    pub tdma: u8,
    pub field_valid: u16,
    pub cur_cyls: u16,
    pub cur_heads: u16,
    pub cur_sectors: u16,
    pub cur_capacity0: u16,
    pub cur_capacity1: u16,
    pub multsect: u8,
    pub multsect_valid: u8,
    pub lba_capacity: u32,
    pub dma_1word: u16,
    pub dma_mword: u16,
    pub eide_pio_modes: u16,
    pub eide_dma_min: u16,
    pub eide_dma_time: u16,
    pub eide_pio: u16,
    pub eide_pio_iordy: u16,
    pub words69_70: [u16; 2],
    pub words71_74: [u16; 4],
    pub queue_depth: u16,
    pub sata_capability: u16,
    pub sata_additional: u16,
    pub sata_supported: u16,
    pub features_enabled: u16,
    pub major_rev_num: u16,
    pub minor_rev_num: u16,
    pub command_set_1: u16,
    pub command_set_2: u16,
    pub cfsse: u16,
    pub cfs_enable_1: u16,
    pub cfs_enable_2: u16,
    pub csf_default: u16,
    pub dma_ultra: u16,
    pub word89: u16,
    pub word90: u16,
    pub cur_amp_values: u16,
    pub word92: u16,
    pub comreset: u16,
    pub accoustic: u16,
    pub min_req_sz: u16,
    pub transfer_time_dma: u16,
    pub access_latency: u16,
    pub perf_granularity: u32,
    total_usr_sectors: [u32; 2],
    pub transfer_time_pio: u16,
    pub reserved105: u16,
    pub sector_sz: u16,
    pub inter_seek_delay: u16,
    pub words108_116: [u16; 9],
    words_per_sector: [u16; 2],
    pub supported_settings: u16,
    pub command_set_3: u16,
    pub words121_126: [u16; 6],
    pub word127: u16,
    pub security_status: SecurityStatus,
    pub csfo: Csf0,
    pub words130_155: [u16; 26],
    pub word156: u16,
    pub words157_159: [u16; 3],
    pub cfa: u16,
    pub words161_175: [u16; 15],
    pub media_serial: [u8; 60],
    pub sct_cmd_transport: u16,
    pub words207_208: [u16; 2],
    pub block_align: u16,
    pub wrv_sec_count: u32,
    pub verf_sec_count: u32,
    pub nv_cache_capability: u16,
    pub nv_cache_sz: u16,
    pub nv_cache_sz2: u16,
    pub rotation_rate: u16,
    pub reserved218: u16,
    pub nv_cache_options: u16,
    pub words220_221: [u16; 2],
    pub transport_major_rev: u16,
    pub transport_minor_rev: u16,
    pub words224_233: [u16; 10],
    pub min_dwnload_blocks: u16,
    pub max_dwnload_blocks: u16,
    pub words236_254: [u16; 19],
    pub integrity: u16,
}

impl IdentifyStructure {
    pub fn words_per_sector(&self) -> u32 {
        (self.words_per_sector[1] as u32) << 16 | self.words_per_sector[0] as u32
    }

    pub fn total_usr_sectors(&self) -> u64 {
        (self.total_usr_sectors[1] as u64) << 32 | self.total_usr_sectors[0] as u64
    }
}

impl Debug for IdentifyStructure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IdentifyStructure")
            .field("track_bytes", &self.track_bytes)
            .field("sector_bytes", &self.sector_bytes)
            .field("sectors per track", &self.sectors)
            .field("total_usr_sectors", &self.total_usr_sectors())
            .field("sector_sz", &self.sector_sz)
            .field("words_per_sector", &self.words_per_sector())
            .field("lba_capacity", &self.lba_capacity) //sectors??
            .field("queue_depth", &self.queue_depth)
            .finish_non_exhaustive()
    }
}

bitfield! {
    pub struct SecurityStatus(u16);
    impl Debug;
    sec_level, _: 8;
    enhanced_erase, _: 5;
    expire, _: 4;
    frozen, _: 3;
    locked, _: 2;
    en_dis, _: 1;
    capability, _: 0;
}
bitfield! {
    pub struct Csf0(u16);
    impl Debug;
    auto_reassign, _: 3;
    reverting, _: 2;
    read_look_ahead, _: 1;
    write_cache, _: 0;
}
