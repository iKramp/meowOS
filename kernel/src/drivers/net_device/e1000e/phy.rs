/*
registers @ page 372 of pdf
*/

use core::time::Duration;
use std::error::ErrorCode;

use crate::drivers::net_device::e1000e::{E1000eDevice, PhyAddress, mdio, registers::MDICPtr};

pub fn init_phy(dev: &mut E1000eDevice) -> Result<(), ErrorCode> {
    set_phy(dev)?;
    let mdic_reg = dev.registers.mdic();
    mdio::modify(&mdic_reg, dev.phy_addr, 0, |val| val | (1 << 15))?; //reset
    while mdio::read(&mdic_reg, dev.phy_addr, 0)? & (1 << 15) != 0 {}
    mdio::modify(&mdic_reg, dev.phy_addr, 0, |val| val | (1 << 12))?; //auto negotiate
    mdio::modify(&mdic_reg, dev.phy_addr, 0, |val| val | (1 << 9))?; //auto negotiate restart
    while mdio::read(&mdic_reg, dev.phy_addr, 1)? & (1 << 5) == 0 {} //wait for auto negotiate complete
    mdio::select_page(&mdic_reg, dev.phy_addr, 0)?;

    //config

    let start = std::time::Instant::now();
    let mut new = std::time::Instant::now();
    let mut status_link_up = mdio::read(&mdic_reg, dev.phy_addr, 17)?;
    while (status_link_up >> 10) & 1 == 0 && new - start < Duration::from_millis(1) {
        status_link_up = mdio::read(&mdic_reg, dev.phy_addr, 17)?;
        new = std::time::Instant::now();
    }
    dev.link_up = (status_link_up >> 10) & 1 == 1;

    //enable link status changed interrupt (detect cable plugs/unplugs)
    mdio::select_page(&mdic_reg, dev.phy_addr, 0)?;
    mdio::write(&mdic_reg, dev.phy_addr, 18, 1 << 10)?;
    mdio::read(&mdic_reg, dev.phy_addr, 17)?;

    Ok(())
}

fn set_phy(dev: &mut E1000eDevice) -> Result<(), ErrorCode> {
    if let Some(id) = get_phy_id(&dev.registers.mdic(), PhyAddress::ExternalGigabit) {
        dev.phy_addr = PhyAddress::ExternalGigabit;
        dev.phy_id = id;
        return Ok(());
    }
    if let Some(id) = get_phy_id(&dev.registers.mdic(), PhyAddress::InternalPCIe) {
        dev.phy_addr = PhyAddress::InternalPCIe;
        dev.phy_id = id;
        return Ok(());
    }
    Err(ErrorCode::NoEntry)
}

fn get_phy_id(mdic_addr: &MDICPtr, addr: PhyAddress) -> Option<u32> {
    let Ok(id1) = mdio::read(mdic_addr, addr, 2) else {
        return None;
    };
    let Ok(id2) = mdio::read(mdic_addr, addr, 3) else {
        return None;
    };
    Some(((id1 as u32) << 16) | (id2 as u32))
}

pub fn get_link_up(dev: &E1000eDevice) -> bool {
    let status = mdio::read(&dev.registers.mdic(), dev.phy_addr, 17).unwrap_or(0);
    (status >> 10) & 1 == 1
}
