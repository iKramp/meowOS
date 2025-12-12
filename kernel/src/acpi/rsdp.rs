use std::mem_utils::{PhysAddr, VirtAddr, ensure_aligned};

use reg_map::RegMap;

pub enum Rsdp {
    V1(RsdpV1Ptr<'static>),
    V2(RsdpV2Ptr<'static>),
}

impl Rsdp {
    fn validate(&self) -> bool {
        match self {
            Self::V1(data) => {
                let mut sum = 0_u16;
                for i in 0..8 {
                    sum += data.signature().idx(i).read() as u16;
                }
                for i in 0..6 {
                    sum += data.oemid().idx(i).read() as u16;
                }
                sum += data.checksum().read() as u16;
                sum += data.revision().read() as u16;
                for i in 0..4 {
                    sum += ((data.rsdt_address().read() >> (i * 8)) & 0xFF) as u16
                }

                (sum & 0xFF) == 0
            }
            Self::V2(data) => {
                let mut sum = 0_u16;
                for i in 0..8 {
                    sum += data.signature().idx(i).read() as u16;
                }
                for i in 0..6 {
                    sum += data.oemid().idx(i).read() as u16;
                }
                sum += data.checksum().read() as u16;
                sum += data.revision().read() as u16;

                for i in 0..4 {
                    sum += ((data.rsdt_address().read() >> (i * 8)) & 0xFF) as u16
                }

                for i in 0..4 {
                    sum += ((data.length().read() >> (i * 8)) & 0xFF) as u16
                }

                for i in 0..8 {
                    sum += ((data.xsdt_address().read() >> (i * 8)) & 0xFF) as u16
                }

                sum += data.extended_checksum().read() as u16;
                sum += data.reserved().idx(0).read() as u16;
                sum += data.reserved().idx(1).read() as u16;
                sum += data.reserved().idx(2).read() as u16;

                (sum & 0xFF) == 0
            }
        }
    }

    pub fn address(&self) -> PhysAddr {
        match self {
            Self::V1(data) => PhysAddr(data.rsdt_address().read() as u64),
            Self::V2(data) => PhysAddr(data.xsdt_address().read()),
        }
    }

    pub fn signature(&self) -> [char; 8] {
        let mut buf = ['a'; 8];
        match self {
            Self::V1(data) => data
                .signature()
                .iter()
                .map(|a| a.read() as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
            Self::V2(data) => data
                .signature()
                .iter()
                .map(|a| a.read() as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
        };
        buf
    }

    pub fn oem_id(&self) -> [char; 6] {
        let mut buf = ['a'; 6];
        match self {
            Self::V1(data) => data
                .oemid()
                .iter()
                .map(|a| a.read() as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
            Self::V2(data) => data
                .oemid()
                .iter()
                .map(|a| a.read() as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
        };
        buf
    }
}

#[repr(C)]
#[derive(Debug, RegMap)]
pub struct RsdpV1 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C)]
#[derive(Debug, RegMap)]
pub struct RsdpV2 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,

    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

//first do memory allocation and mapping, then i can map rsdp memory and do this
pub fn get_rsdp_table(rsdp_addr: u64) -> Option<Rsdp> {
    //guard against misaligned tables...
    let rsdp_addr = unsafe { ensure_aligned::<RsdpV2>(VirtAddr(rsdp_addr)).0 };

    let rsdp_table = unsafe { RsdpV1Ptr::from_ptr(rsdp_addr as *mut _) };
    let revision = rsdp_table.revision().read();
    let rsdp = if revision == 0 {
        Rsdp::V1(rsdp_table)
    } else {
        let rsdp_table_v2 = unsafe { RsdpV2Ptr::from_ptr(rsdp_addr as *mut _) };
        Rsdp::V2(rsdp_table_v2)
    };

    if !rsdp.validate() {
        return None;
    }

    Some(rsdp)
}
