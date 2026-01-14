use bitfield::bitfield;
use reg_map::RegMap;

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct ICR(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct ITR(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct ICS(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct IMS(u32);
    impl Debug;
}

bitfield! {
    #[derive(RegMap)]
    pub(in crate::drivers::net_device::e1000e) struct IMC(u32);
    impl Debug;
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
