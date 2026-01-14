use reg_map::RegMap;

use general::*;
use interrupt::*;

mod general;
mod interrupt;


#[derive(Debug, RegMap)]
#[repr(C)]
pub(super) struct E1000eRegisters {
    #[reg(RW)] pub ctrl: CTRL,
    #[reg(RW)] ctrl_duplicate: CTRL,
    #[reg(RO)] pub status: STATUS,
    reserved_0: u32,
    #[reg(RW)] pub eec: EEC,
    #[reg(RW)] pub eerd: EERD,
    #[reg(RW)] pub ctrl_ext: CTRL_EXT,
    #[reg(RW)] pub fla: FLA,
    #[reg(RW)] pub mdic: MDIC,
    reserved_1: u32,
    #[reg(RW)] pub fcal: FCAL,
    #[reg(RW)] pub fcah: FCAH,
    #[reg(RW)] pub fct: FCT,
    reserved_2: u32,
    #[reg(RW)] pub vet: VET,
    reserved_3: [u8; 0x84],
    #[reg(RW)] pub icr: ICR,
    #[reg(RW)] pub itr: ITR,
    #[reg(WO)] pub ics: ICS,
    reserved_4: u32,
    #[reg(RW)] pub ims: IMS,
    reserved_5: u32,
    #[reg(WO)] pub imc: IMC,
    #[reg(RW)] pub eiac: EIAC,
    #[reg(RW)] pub iam: IAM,
    #[reg(RW)] pub ivar: IVAR,
    reserved_6: [u8; 0x88],
    #[reg(RW)] pub fcttv: FCTTV,
    reserved_7: [u8; 0xC8C],
    #[reg(RW)] pub ledctl: LEDCTL,
    reserved_8: [u8; 0xFC],
    #[reg(RW)] pub extcnf_ctrl: EXTCNF_CTRL,
    reserved_9: u32,
    #[reg(RW)] pub extcnf_size: EXTCNF_SIZE,
    reserved_10: [u8; 0xF4],
    #[reg(RW)] pub pba: PBA,
    reserved_11: [u8; 12],
    #[reg(RO)] pub eemngctl: EEMNGCTL,
    #[reg(RO)] pub eemngdata: EEMNGDATA,
    #[reg(RO)] pub flmngctl: FLMNGCTL,
    #[reg(RO)] pub flmngdata: FLMNGDATA,
    #[reg(RO)] pub flmngcnt: FLMNGCNT,
    reserved_12: u32,
    #[reg(RW)] pub flasht: FLASHT,
    #[reg(RW)] pub eewr: EEWR,
    #[reg(RW)] pub flswctl: FLSWCTL,
    #[reg(RW)] pub flswdata: FLSWDATA,
    #[reg(RW)] pub flswcnt: FLSWCNT,
    #[reg(RW)] pub flop: FLOP,
    reserved_13: [u8; 0x10],
    #[reg(RW)] pub flol: FLOL,
    reserved_14: [u8; 0x4EEC],
    #[reg(RW)] pub fcrtv: FCRTV, //offset 0x5f40
}
