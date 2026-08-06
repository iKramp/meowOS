use std::error::KernelError;

use crate::drivers::net_device::e1000e::registers::E1000eRegistersPtr;

pub(super) fn init_statistics(_dev: &E1000eRegistersPtr) -> Result<(), KernelError> {
    //unused until statistics are actually used
    Ok(())
}
