use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct TCTL(u32);
    impl Debug;
    pub rrtresh, set_rrtresh: 30, 29;
    pub mulr, set_mulr: 28;
    pub txdscmt, set_txdscmt: 27, 26;
    pub unortx, set_unortx: 25;
    pub rtlc, set_rtlc: 24;
    pub pbe, set_pbe: 23;
    pub swxoff, set_swxoff: 22;
    pub cold, set_cold: 21, 12;
    pub ct, set_ct: 11, 4;
    pub psp, set_psp: 3;
    pub en, set_en: 1;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct TIPG(u32);
    impl Debug;
    pub ipgr2, set_ipgr2: 29, 20;
    pub ipgr1, set_ipgr1: 19, 10;
    pub ipgt, set_ipgt: 9, 0;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct TARC(u32);
    impl Debug;
    pub en, set_en: 10;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct TXDCTL(u32);
    impl Debug;
    pub lwtresh, set_lwthresh: 31, 25;
    pub gran, set_gran: 24;
    pub wthresh, set_wthresh: 21, 16;
    pub hthresh, set_hthresh: 13, 8;
    pub pthresh, set_pthresh: 5, 0;
}

#[derive(Debug, Clone, Copy, RegMap)]
#[repr(C)]
pub(in crate::drivers::net_device::e1000e) struct TransmitDescriptorQueueInfo {
    pub tdbal: u32,
    pub tdbah: u32,
    pub tdlen: u32,
    reserved_0: u32,
    pub tdh: u32,
    reserved_1: u32,
    pub tdt: u32,
    reserved_2: u32,
    reserced_3: [u64; 1],
    pub txdctl: TXDCTL,
    reserved_4: u32,
    reserced_5: [u64; 2],
    pub tarc: TARC,
}
