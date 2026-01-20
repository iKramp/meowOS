use bitfield::bitfield;
use reg_map::RegMap;


bitfield! {
    #[derive(RegMap)]
    pub struct GCR2(u32);
    impl Debug;
    pub must_set_1, set_must_set_1: 0;
}

//a bunch of other things
bitfield! {
    #[derive(RegMap)]
    pub struct GCR(u32);
    impl Debug;
    pub must_set_1, set_must_set_1: 22;
}
