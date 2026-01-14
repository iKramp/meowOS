#![allow(non_camel_case_types)]

use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct CTRL(u32);
    impl Debug;
    phy_rst, set_phy_rst: 31;
    ///VLAN mode enable
    vme, set_vme: 30;
    ///transmit flow control enable
    tfce, set_tfce: 28;
    ///receive flow control enable
    rfce, set_rfce: 27;
    rst, set_rst: 26;
    ///D3Cold wakeup capability advertisement on AUX_PWR
    advd3wuc, _: 20;
    ///force duplex
    frcdplx, set_frcdplx: 12;
    ///force speed
    frcspd, set_frcspd: 11;
    speed_internal, set_speed_internal: 9,8;
    ///set link up
    slu, set_slu: 6;
    ///auto speed detection enable
    asde, set_asde: 5;
    gio_master_disable, set_gio_master_disable: 2;
    ///full duplex
    fd, set_fd: 0;
}

pub(in crate::drivers::net_device::e1000e) enum EtherLinkSpeed {
    SPEED_10MBPS = 0b00,
    SPEED_100MBPS = 0b01,
    SPEED_1GBPS = 0b10,
}

impl From<u32> for EtherLinkSpeed {
    fn from(value: u32) -> Self {
        match value {
            0b00 => EtherLinkSpeed::SPEED_10MBPS,
            0b01 => EtherLinkSpeed::SPEED_100MBPS,
            0b10 => EtherLinkSpeed::SPEED_1GBPS,
            _ => panic!("Invalid speed value"),
        }
    }
}

impl CTRL {
    pub fn set_speed(&mut self, speed: EtherLinkSpeed) {
        self.set_speed_internal(speed as u32);
    }
    pub fn speed(&self) -> EtherLinkSpeed {
        self.speed_internal().into()
    }
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
