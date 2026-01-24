use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub struct RCTL(u32);
    impl Debug;
    pub flxbuf, set_flxbuf: 30,27;
    pub secrc, set_secrc: 26;
    pub bsex, set_bsex: 25;
    pub pmcf, set_pmcf: 23;
    pub dpf, set_dpf: 22;
    pub cfi, set_cfi: 20;
    pub cfien, set_cfien: 19;
    pub vfe, set_vfe: 18;
    pub bsize, set_bsize: 17,16;
    pub bam, set_bam: 15;
    pub mo, set_mo: 13,12;
    pub dtyp, set_dtyp: 11,10;
    pub rdmts, set_rdmts: 9,8;
    pub lbm, set_lbm: 7,6;
    pub lpe, set_lpe: 5;
    pub mpe, set_mpe: 4;
    pub upe, set_upe: 3;
    pub sbp, set_sbp: 2;
    pub en, set_en: 1;
}

bitfield! {
    #[derive(RegMap)]
    pub struct PSRCTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCRTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct FCRTH(u32);
    impl Debug;
}

#[derive(Debug, Clone, Copy, RegMap)]
#[repr(C)]
pub struct ReceiveDescriptorQueueInfo {
    pub rdbal: u32,
    pub rdbah: u32,
    pub rdlen: u32,
    reserved_0: u32,
    pub rdh: u32,
    reserved_1: u32,
    pub rdt: u32,
    reserved_2: u32,
}

bitfield! {
    #[derive(RegMap)]
    pub struct RDTR(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RXDCTL(u32);
    impl Debug;
    pub gran, set_gran: 24;
    pub wthresh, set_wthresh: 21,16;
    pub htresh, set_hthresh: 13,8;
    pub pthresh, set_pthresh: 5,0;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RADV(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RSRPD(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RAID(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RXCSUM(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RFCTL(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct MAVTV0(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct MAVTV1(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct MAVTV2(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct MAVTV3(u32);
    impl Debug;
}

#[derive(Debug, Clone, Copy, RegMap)]
#[repr(C)]
pub(in crate::drivers::net_device::e1000e) struct ReceiveAddress {
    pub ral: u32,
    pub rah: u32,
}

bitfield! {
    #[derive(RegMap)]
    pub struct MRQC(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RETA(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub struct RSSRK(u32);
    impl Debug;
}
