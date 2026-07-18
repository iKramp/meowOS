use crate::memory::addresses::*;

pub enum Rsdp {
    V1(&'static RsdpV1),
    V2(&'static RsdpV2),
}

impl Rsdp {
    fn validate(&self) -> bool {
        match self {
            Self::V1(data) => {
                let mut sum = 0_u16;
                for i in 0..8 {
                    sum += data.signature[i] as u16;
                }
                for i in 0..6 {
                    sum += data.oemid[i] as u16;
                }
                sum += data.checksum as u16;
                sum += data.revision as u16;
                for i in 0..4 {
                    sum += ((data.rsdt_address >> (i * 8)) & 0xFF) as u16
                }

                (sum & 0xFF) == 0
            }
            Self::V2(data) => {
                let mut sum = 0_u16;
                for i in 0..8 {
                    sum += data.signature[i] as u16;
                }
                for i in 0..6 {
                    sum += data.oemid[i] as u16;
                }
                sum += data.checksum as u16;
                sum += data.revision as u16;

                for i in 0..4 {
                    sum += ((data.rsdt_address >> (i * 8)) & 0xFF) as u16
                }

                for i in 0..4 {
                    sum += ((data.length >> (i * 8)) & 0xFF) as u16
                }

                for i in 0..8 {
                    sum += ((data.xsdt_address >> (i * 8)) & 0xFF) as u16
                }

                sum += data.extended_checksum as u16;
                sum += data.reserved[0] as u16;
                sum += data.reserved[1] as u16;
                sum += data.reserved[2] as u16;

                (sum & 0xFF) == 0
            }
        }
    }

    pub fn address(&self) -> PhysAddr {
        match self {
            Self::V1(data) => PhysAddr(data.rsdt_address as u64),
            Self::V2(data) => PhysAddr(data.xsdt_address),
        }
    }

    pub fn signature(&self) -> [char; 8] {
        let mut buf = ['a'; 8];
        match self {
            Self::V1(data) => data
                .signature
                .iter()
                .map(|a| *a as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
            Self::V2(data) => data
                .signature
                .iter()
                .map(|a| *a as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
        };
        buf
    }

    pub fn oem_id(&self) -> [char; 6] {
        let mut buf = ['a'; 6];
        match self {
            Self::V1(data) => data
                .oemid
                .iter()
                .map(|a| *a as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
            Self::V2(data) => data
                .oemid
                .iter()
                .map(|a| *a as char)
                .enumerate()
                .for_each(|(i, c)| buf[i] = c),
        };
        buf
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct RsdpV1 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C)]
#[derive(Debug)]
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
    let rsdp_addr = unsafe { align::<RsdpV2>(VirtAddr(rsdp_addr)).0 };

    let rsdp_table = unsafe { &mut *(rsdp_addr as *mut RsdpV1) };
    let revision = rsdp_table.revision;
    let rsdp = if revision == 0 {
        Rsdp::V1(rsdp_table)
    } else {
        let rsdp_table_v2 = unsafe { &mut *(rsdp_addr as *mut RsdpV2) };
        Rsdp::V2(rsdp_table_v2)
    };

    if !rsdp.validate() {
        return None;
    }

    Some(rsdp)
}
