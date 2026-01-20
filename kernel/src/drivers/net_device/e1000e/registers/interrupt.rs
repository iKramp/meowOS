#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case)]

use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct InterruptMask(u32);
    impl Debug;
    pub int_asserted, set_int_asserted: 31;
    pub other, set_other: 24;
    ///Transmit queue 1
    pub TxQ1, set_TxQ1: 23;
    ///Transmit queue 0
    pub TxQ0, set_TxQ0: 22;
    ///Receive queue 1
    pub RxQ1, set_RxQ1: 21;
    ///Receive queue 0
    pub RxQ0, set_RxQ0: 20;
    ///Manageability event
    pub MNG, set_MNG: 18;
    ///Receive ACK frame
    pub ACK, set_ACK: 17;
    ///Small receive packet
    pub SRPD, set_SRPD: 16;
    ///Transmit descriptor low treshold
    pub TXD_LOW, set_TXD_LOW: 15;
    ///MDIO access complete
    pub MDAC, set_MDAC: 9;
    ///Receiver timer interrupt
    pub RXT0, set_RXT0: 7;
    ///Receiver overrun
    pub RXO, set_RXO: 6;
    ///Receive descriptor minimum threshold
    pub RXDMT0, set_RXDMT0: 5;
    ///Link status change
    pub LSC, set_LSC: 2;
    ///Transmit queue empty
    pub TXQE, set_TXQE: 1;
    ///Transmit descriptor written back
    pub TXDW, set_TXDW: 0;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct ITR(u32);
    impl Debug;
    interval, set_interval: 15,0;
}

impl ITR {
    pub fn get_interval_ns(&self) -> u32 {
        //each unit is 256ns
        self.interval() * 256
    }
    pub fn set_interval_ns(&mut self, ns: u32) {
        let units = ns / 256;
        self.set_interval(units);
    }
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct EIAC(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct IAM(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct IVAR(u32);
    impl Debug;
}
