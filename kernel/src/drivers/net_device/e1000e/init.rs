use std::{println, w_lock_w_info};

use crate::drivers::net_device::e1000e::{
    E1000eDevice, phy, receive::{disable_receive, enable_receive, init_receive}, registers::{FCAH, FCAL, FCT, InterruptMask}, transmit::{disable_transmit, enable_transmit, init_transmit}
};

pub(super) fn init(dev: &mut E1000eDevice) {
    let registers = w_lock_w_info!(dev.registers);
    //disable all interrupts
    registers.imc().write(all_interrupts_mask());

    //reset
    registers.ctrl().write(*registers.ctrl().read().set_rst(true));
    std::thread::sleep(std::time::Duration::from_millis(1));
    while registers.ctrl().read().rst() {}
    println!("E1000e reset complete");

    //disable interrupts again
    registers.imc().write(all_interrupts_mask());

    registers.ctrl().write(
        *registers
            .ctrl()
            .read()
            .set_asde(false)
            .set_frcdplx(false)
            .set_frcspd(false)
            .set_slu(true)
    );

    //no flow control for now
    registers.fcah().write(FCAH(0));
    registers.fcal().write(FCAL(0));
    registers.fct().write(FCT(0));
    registers.gcr().write(*registers.gcr().read().set_must_set_1(true));
    registers.gcr2().write(*registers.gcr2().read().set_must_set_1(true));

    drop(registers);

    //init phy
    if let Err(e) = phy::init_phy(dev) {
        println!("Failed to initialize PHY: {}", e);
        return;
    }
    println!("PHY initialized");

    //init stats

    init_receive(dev);
    init_transmit(dev);
    println!("Receive and Transmit initialized");

    enable_receive(dev);
    enable_transmit(dev);

    let registers = w_lock_w_info!(dev.registers);
    //enable interesting interrupts
    registers.icr().read(); //clear all first
    registers.ims().write(interesting_interrupts_mask());
    drop(registers);

    println!(level:info, "E1000e initialized successfully");
    let mac_address = dev.mac_address.0;
    println!(
        "MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5]
    );
    //print link up
    println!("Link up: {}", phy::get_link_up(dev));

    println!("Enabling promiscuous mode for testing purposes");
    dev.enable_promiscuous_mode();
}

pub(super) fn deinit(dev: &mut E1000eDevice) {
    w_lock_w_info!(dev.registers).imc().write(all_interrupts_mask());
    disable_receive(dev);
    disable_transmit(dev);
    let registers = w_lock_w_info!(dev.registers);
    registers.ctrl().write(*registers.ctrl().read().set_slu(false)); //link down
}

//everything except reserved fields
fn all_interrupts_mask() -> InterruptMask {
    InterruptMask(0x81F782D7)
}

fn interesting_interrupts_mask() -> InterruptMask {
    *InterruptMask(0).set_RXT0(true).set_RXO(true).set_LSC(true).set_RXDMT0(true).set_RxQ0(true)
}
