use std::{error::ErrorCode, println, w_lock_w_info};

use crate::{drivers::net_device::e1000e::{E1000eDevice, registers::E1000eRegistersPtr}};



pub(super) fn write_nvm(dev: &E1000eRegistersPtr, addr: u16, data: u16) {
    dev.eewr().write(
        *dev.eewr()
            .read()
            .set_addr(addr as u32)
            .set_data(data as u32)
            .set_start(true)
    );
    while !dev.eewr().read().done() {}
}

pub (super) fn read_nvm(dev: &E1000eRegistersPtr, addr: u16) -> u16 {
    dev.eerd().write(
        *dev.eerd()
            .read()
            .set_addr(addr as u32)
            .set_start(true)
    );
    while !dev.eewr().read().done() {}
    dev.eewr().read().data() as u16
}

pub(super) enum NvmState {
    Changed,
    Unchanged,
}

fn config_nvm(dev: &mut E1000eDevice) -> Result<NvmState, ErrorCode> {
    let registers = w_lock_w_info!(dev.registers);
    let mut state = NvmState::Unchanged;
    let mut ecc = registers.eec().read();
    println!("{:X?}", ecc);

    if ecc.nvadds() == 0 {
        println!("NVM address size is 0");
        if ecc.nvmtype() {
            //flash
            println!(level:error, "e1000e config_nvm error: NVM type is flash and nvsize is 0");
            return Err(ErrorCode::IllegalValue);
        } else {
            //eeprom
            let size: u32 = 128 * (1 << ecc.nvsize());
            if size > 1 << 16 {
                println!(level:error, "e1000e config_nvm error: Invalid NVM size: {}", size);
                return Err(ErrorCode::IllegalValue);
            }
            let addr_size = if size > 1 << 8 {
                2
            } else {
                1
            };
            println!("NVM size: {} bytes", size);
            registers.eec().write(*ecc.set_nvadds(addr_size));
            state = NvmState::Changed;
        }
    }



    //read mac
    let bytes_1 = read_nvm(&registers, 0);
    let bytes_2 = read_nvm(&registers, 1);
    let bytes_3 = read_nvm(&registers, 2);
    dev.mac_address = [
        (bytes_1 & 0xFF) as u8,
        (bytes_1 >> 8) as u8,
        (bytes_2 & 0xFF) as u8,
        (bytes_2 >> 8) as u8,
        (bytes_3 & 0xFF) as u8,
        (bytes_3 >> 8) as u8,
    ];
    //print
    println!(
        "E1000e NVM MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        dev.mac_address[0],
        dev.mac_address[1],
        dev.mac_address[2],
        dev.mac_address[3],
        dev.mac_address[4],
        dev.mac_address[5],
    );

    Ok(state)
}
