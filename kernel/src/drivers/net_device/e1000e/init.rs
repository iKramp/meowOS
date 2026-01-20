use std::println;

use crate::drivers::net_device::e1000e::{
    E1000eDevice,
    nvm::init_nvm,
    phy,
    receive::{disable_receive, enable_receive, init_receive},
    registers::{FCAH, FCAL, FCT, InterruptMask},
    transmit::{disable_transmit, enable_transmit, init_transmit},
};

pub(super) fn init(dev: &mut E1000eDevice) {
    //disable all interrupts
    dev.registers.imc().write(all_interrupts_mask());

    //reset
    dev.registers.ctrl().write(*dev.registers.ctrl().read().set_rst(true));
    std::thread::sleep(std::time::Duration::from_micros(1));
    while dev.registers.ctrl().read().rst() {}

    //disable interrupts again
    dev.registers.imc().write(all_interrupts_mask());

    dev.registers.ctrl().write(
        *dev.registers
            .ctrl()
            .read()
            .set_asde(false) //docs say must be set to 0
            .set_frcdplx(false)
            .set_frcspd(false)
            .set_slu(true),
    );

    //no flow control for now
    dev.registers.fcah().write(FCAH(0));
    dev.registers.fcal().write(FCAL(0));
    dev.registers.fct().write(FCT(0));
    dev.registers.gcr().write(*dev.registers.gcr().read().set_must_set_1(true));
    dev.registers.gcr2().write(*dev.registers.gcr2().read().set_must_set_1(true));

    //init phy
    if let Err(e) = phy::init_phy(dev) {
        println!("Failed to initialize PHY: {}", e);
        return;
    }

    //init stats

    //nvm init
    init_nvm(dev);

    init_receive(dev);
    init_transmit(dev);

    //enable interesting interrupts
    dev.registers.ims().write(interesting_interrupts_mask());
    enable_receive(dev);
    enable_transmit(dev);

    println!("E1000e initialized successfully");
}

pub(super) fn deinit(dev: &mut E1000eDevice) {
    dev.registers.imc().write(all_interrupts_mask());
    disable_receive(dev);
    disable_transmit(dev);
    dev.registers.ctrl().write(*dev.registers.ctrl().read().set_slu(false)); //link down
}

//everything except reserved fields
fn all_interrupts_mask() -> InterruptMask {
    InterruptMask(0x81F782D7)
}

fn interesting_interrupts_mask() -> InterruptMask {
    InterruptMask(0x100084)
}

//0b0000_0000_0001_0000_0000_0000_1000_0100
