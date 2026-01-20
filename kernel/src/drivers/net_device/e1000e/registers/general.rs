#![allow(non_camel_case_types)]

use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct CTRL(u32);
    impl Debug;
    pub phy_rst, set_phy_rst: 31;
    ///VLAN mode enable
    pub vme, set_vme: 30;
    ///transmit flow control enable
    pub tfce, set_tfce: 28;
    ///receive flow control enable
    pub rfce, set_rfce: 27;
    pub rst, set_rst: 26;
    ///D3Cold wakeup capability advertisement on AUX_PWR
    pub advd3wuc, _: 20;
    ///force duplex
    pub frcdplx, set_frcdplx: 12;
    ///force speed
    pub frcspd, set_frcspd: 11;
    speed_internal, set_speed_internal: 9,8;
    ///set link up
    pub slu, set_slu: 6;
    ///auto speed detection enable
    pub asde, set_asde: 5;
    pub gio_master_disable, set_gio_master_disable: 2;
    ///full duplex
    pub fd, set_fd: 0;
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
    pub fn set_speed(&mut self, speed: EtherLinkSpeed) -> &mut Self {
        self.set_speed_internal(speed as u32)
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
    pub nvmtype, _: 23;
    pub sec1val, _: 22;
    pub aupden, set_aupden: 20;
    pub nvadds, set_nvadds: 16,15;
    pub nvsize, _: 14,11;
    pub auto_rd, _: 9;
    pub ee_pres, _: 8;
    pub ee_gnt, _: 7;
    pub ee_req, set_ee_req: 6;
    pub fwe, set_fwe: 5,4;
    pub ee_do, _: 3;
    pub ee_di, set_ee_di: 2;
    pub ee_cs, set_ee_cs: 1;
    pub ee_sk, set_ee_sk: 0;
}

bitfield! {
    #[derive(RegMap)]
    pub struct EERD(u32);
    impl Debug;
    pub data, _: 31,16;
    pub addr, set_addr: 15,2;
    pub done, _: 1;
    pub start, set_start: 0;
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

#[derive(Debug, Clone, Copy)]
pub enum PhyAddress {
    ExternalGigabit = 1,
    InternalPCIe = 2,
}

pub enum MDICCommand {
    Read = 0b10,
    Write = 0b01,
}

bitfield! {
    #[derive(RegMap)]
    pub struct MDIC(u32);
    impl Debug;
    pub error, set_error: 30;
    pub interrupt_enable, set_interrupt_enable: 29;
    pub ready, set_ready: 28;
    command_internal, set_command_internal: 27,26;
    phy_address_internal, set_phy_address_internal: 25,21;
    pub reg_address, set_reg_address: 20,16;
    pub data, set_data: 15,0;
}

impl MDIC {
    pub fn set_command(&mut self, command: MDICCommand) -> &mut Self {
        self.set_command_internal(command as u32)
    }
    pub fn command(&self) -> MDICCommand {
        match self.command_internal() {
            0b10 => MDICCommand::Read,
            0b01 => MDICCommand::Write,
            _ => panic!("Invalid MDIC command"),
        }
    }

    pub fn set_phy_address(&mut self, addr: PhyAddress) -> &mut Self {
        self.set_phy_address_internal(addr as u32)
    }
    pub fn phy_address(&self) -> PhyAddress {
        match self.phy_address_internal() {
            1 => PhyAddress::ExternalGigabit,
            2 => PhyAddress::InternalPCIe,
            _ => panic!("Invalid MDIC phy address"),
        }
    }
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
    pub data, set_data: 31,16;
    pub addr, set_addr: 15,2;
    pub done, _: 1;
    pub start, set_start: 0;
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
