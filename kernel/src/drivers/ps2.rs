use std::{lock_w_info, sync::no_int_spinlock::NoIntSpinlock, vec::Vec};

use crate::{
    keyboard::KeyboardState,
    utils::{byte_from_port, byte_to_port},
};

static PS2_WRITE_LOCK: NoIntSpinlock<()> = NoIntSpinlock::new(());

pub static mut PS2_KEYBOARD_STATE: KeyboardState = KeyboardState(0); //so rare we don't care about locking

fn wait_for_empty_write_buffer() {
    while byte_from_port(0x64) & 0x2 != 0 {}
}

pub fn reset_cpu() -> ! {
    let lock = lock_w_info!(PS2_WRITE_LOCK);
    wait_for_empty_write_buffer();
    byte_to_port(0x64, 0xFE);
    drop(lock);
    loop {
        //wait for cpu reset
        unsafe { core::arch::asm!("hlt") };
    }
}

//may only be called from interrupt handler
pub fn read_scancodes() -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        if byte_from_port(0x64) & 0x1 == 0 {
            break; //no more data to read
        }
        let byte = byte_from_port(0x60);
        bytes.push(byte);
    }
    bytes
}

pub fn handle_ps2_keyboard_interrupt() {
    let scancodes = crate::drivers::ps2::read_scancodes();
    let keeb_state = unsafe { &mut PS2_KEYBOARD_STATE };
    let keys = crate::keyboard::handle_keyboard_data(scancodes, keeb_state);
    crate::tty::handle_input(keys.into_boxed_slice(), keeb_state);
}
