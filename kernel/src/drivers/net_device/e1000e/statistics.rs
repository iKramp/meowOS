use std::error::ErrorCode;

use crate::drivers::net_device::e1000e::registers::E1000eRegistersPtr;


pub(super) fn init_statistics(_dev: &E1000eRegistersPtr) -> Result<(), ErrorCode> {
    //unused until statistics are actually used
    Ok(())
}
