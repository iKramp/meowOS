use core::time::Duration;
use std::{error::KernelError, kerror};

use crate::drivers::net_device::e1000e::registers::{MDIC, MDICPtr, PhyAddress};

pub(super) fn read(mdic_ptr: &MDICPtr, phy: PhyAddress, reg: u32) -> Result<u16, KernelError> {
    let mut mdic = MDIC(0);
    mdic.set_ready(false);
    mdic.set_interrupt_enable(false);
    mdic.set_command(super::registers::MDICCommand::Read);
    mdic.set_phy_address(phy);
    mdic.set_reg_address(reg);
    mdic.set_error(false);
    mdic_ptr.write(mdic);

    while !mdic.ready() {
        std::thread::sleep(Duration::from_micros(50));
        mdic = mdic_ptr.read();
    }

    if mdic.error() {
        return kerror!(Unknown);
    }

    Ok(mdic.data() as u16)
}

pub(super) fn write(mdic_ptr: &MDICPtr, phy: PhyAddress, reg: u32, data: u16) -> Result<(), KernelError> {
    let mut mdic = MDIC(0);
    mdic.set_ready(false);
    mdic.set_interrupt_enable(false);
    mdic.set_command(super::registers::MDICCommand::Write);
    mdic.set_phy_address(phy);
    mdic.set_reg_address(reg);
    mdic.set_error(false);
    mdic.set_data(data as u32);
    mdic_ptr.write(mdic);

    while !mdic.ready() {
        std::thread::sleep(Duration::from_micros(50));
        mdic = mdic_ptr.read();
    }

    if mdic.error() {
        return kerror!(Unknown);
    }
    Ok(())
}

pub(super) fn modify<F>(mdic_ptr: &MDICPtr, phy: PhyAddress, reg: u32, f: F) -> Result<(), KernelError>
where
    F: FnOnce(u16) -> u16,
{
    let val = read(mdic_ptr, phy, reg)?;
    let new_val = f(val);
    write(mdic_ptr, phy, reg, new_val)
}

pub(super) fn select_page(mdic_ptr: &MDICPtr, phy: PhyAddress, page: u8) -> Result<(), KernelError> {
    write(mdic_ptr, phy, 22, page.into())
}
