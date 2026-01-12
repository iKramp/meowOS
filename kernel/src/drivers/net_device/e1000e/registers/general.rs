#![allow(non_camel_case_types)]

use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub struct CTRL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct STATUS(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EEC(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EERD(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct CTRL_EXT(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLA(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct MDIC(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCAL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCAH(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCT(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct VET(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCTTV(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCRTV(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct LEDCTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EXTCNF_CTRL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EXTCNF_SIZE(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct PBA(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EEMNGCTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EEMNGDATA(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLMNGCTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLMNGDATA(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLMNGCNT(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLASHT(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EEWR(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLSWCTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLSWDATA(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLSWCNT(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLOP(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FLOL(u32);
    impl Debug;
}
