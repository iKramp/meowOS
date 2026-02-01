use core::mem::offset_of;

use reg_map::RegMap;

pub(super) use general::*;
pub(super) use interrupt::*;
pub(super) use pcie::*;
pub(super) use receive::*;
pub(super) use transmit::*;


mod general;
mod interrupt;
mod pcie;
mod receive;
mod transmit;


#[derive(Debug, RegMap)]
#[repr(C)]
pub(super) struct E1000eRegisters {
    #[reg(RW)] pub ctrl: CTRL,
    #[reg(RW)] ctrl_duplicate: CTRL,
    #[reg(RO)] pub status: STATUS,
    reserved_eec: u32,
    #[reg(RW)] pub eec: EEC,
    #[reg(RW)] pub eerd: EERD,
    #[reg(RW)] pub ctrl_ext: CTRL_EXT,
    #[reg(RW)] pub fla: FLA,
    #[reg(RW)] pub mdic: MDIC,
    reserved_fcal: u32,
    #[reg(RW)] pub fcal: FCAL,
    #[reg(RW)] pub fcah: FCAH,
    #[reg(RW)] pub fct: FCT,
    reserved_vet: u32,
    #[reg(RW)] pub vet: VET,
    reserved_icr: [u8; 0x84],
    #[reg(RW)] pub icr: InterruptMask,
    #[reg(RW)] pub itr: ITR,
    #[reg(WO)] pub ics: InterruptMask,
    reserved_ims: u32,
    #[reg(RW)] pub ims: InterruptMask,
    reserved_imc: u32,
    #[reg(WO)] pub imc: InterruptMask,
    #[reg(RW)] pub eiac: EIAC,
    #[reg(RW)] pub iam: IAM,
    #[reg(RW)] pub ivar: IVAR,
    reserved_rctl: [u8; 0x18],
    #[reg(RW)] pub rctl: RCTL,
    reserved_fcttv: [u8; 0x6C],
    #[reg(RW)] pub fcttv: FCTTV,
    reserved_tctl: [u8; 0x28C],
    #[reg(RW)] pub tctl: TCTL,
    reserved_tipg: [u32; 3],
    #[reg(RW)] pub tipg: TIPG,
    reserved_ledctl: [u8; 0x9EC],
    #[reg(RW)] pub ledctl: LEDCTL,
    reserved_extcnf_ctrl: [u8; 0xFC],
    #[reg(RW)] pub extcnf_ctrl: EXTCNF_CTRL,
    reserved_extcnf_size: u32,
    #[reg(RW)] pub extcnf_size: EXTCNF_SIZE,
    reserved_pba: [u8; 0xF4],
    #[reg(RW)] pub pba: PBA,
    reserved_eemngctl: [u8; 12],
    #[reg(RO)] pub eemngctl: EEMNGCTL,
    #[reg(RO)] pub eemngdata: EEMNGDATA,
    #[reg(RO)] pub flmngctl: FLMNGCTL,
    #[reg(RO)] pub flmngdata: FLMNGDATA,
    #[reg(RO)] pub flmngcnt: FLMNGCNT,
    reserved_flasht: u32,
    #[reg(RW)] pub flasht: FLASHT,
    #[reg(RW)] pub eewr: EEWR,
    #[reg(RW)] pub flswctl: FLSWCTL,
    #[reg(RW)] pub flswdata: FLSWDATA,
    #[reg(RW)] pub flswcnt: FLSWCNT,
    #[reg(RW)] pub flop: FLOP,
    reserved_flol: [u8; 0x10],
    #[reg(RW)] pub flol: FLOL,
    reserved_fcrtl: [u8; 0x110C],
    #[reg(RW)] pub fcrtl: FCRTL,
    reserved_fcrth: u32,
    #[reg(RW)] pub fcrth: FCRTH,
    reserved_psrctl: u32,
    #[reg(RW)] pub psrctl: PSRCTL,
    reserved_rdbal0: [u8; 0x68C],
    #[reg(RW)] pub rx_descriptor_queue_info: ReceiveDescriptorQueueInfo,
    #[reg(RW)] pub rdtr: RDTR,
    reserved_rxdctl: u32,
    #[reg(RW)] pub rxdctl: RXDCTL,
    #[reg(RW)] pub radv: RADV,
    reserved_rsrpd: [u8; 0x3D0],
    #[reg(RW)] pub rsrpd: RSRPD,
    reserved_raid: u32,
    #[reg(RW)] pub raid: RAID,
    reserved_t0dctl: [u8; 0xBF4],
    #[reg(RW)] pub tx_descriptor_queue_info: TransmitDescriptorQueueInfo,
    reserved_rxcsum: [u8; 0x17B8],
    #[reg(RW)] pub rxcsum: RXCSUM,
    reserved_rfctl: u32,
    #[reg(RW)] pub rfctl: RFCTL,
    reserved_mavtv: u32,
    #[reg(RW)] pub mavtv0: MAVTV0,
    #[reg(RW)] pub mavtv1: MAVTV1,
    #[reg(RW)] pub mavtv2: MAVTV2,
    #[reg(RW)] pub mavtv3: MAVTV3,
    reserved_mta: [u8; 0x1E0],
    #[reg(RW)] pub mta: [u32; 128],
    #[reg(RW)] pub rx_add: [ReceiveAddress; 16],
    reserved_vfta: [u8; 0x180],
    #[reg(RW)] pub vfta: [u32; 128],
    reserved_mrqc: [u8; 0x18],
    #[reg(RW)] pub mrqc: MRQC,
    reserved_gcr: [u8; 0x2E4],
    #[reg(RW)] pub gcr: GCR,
    reserved_gcr_2: [u8; 0x60],
    #[reg(RW)] pub gcr2: GCR2,
    reserved_reta: [u8; 0x98],
    #[reg(RW)] pub reta: [u32; 32],
    #[reg(RW)] pub rssrk: [u32; 10],
    reserved_fcrtv: [u8; 0x298],
    #[reg(RW)] pub fcrtv: FCRTV, //offset 0x5f40
}

unsafe impl Send for E1000eRegistersPtr<'_> {}

const _: () = {
    assert!(offset_of!(E1000eRegisters, fcrtv) == 0x5f40, "E1000eRegs.fcrtv offset is wrong!");
};
