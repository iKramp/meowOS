use std::{lock_w_info, println, sync::no_int_spinlock::NoIntSpinlock, vec::Vec};

use crate::{
    keyboard::KeyboardState,
    utils::{byte_from_port, byte_to_port},
};

struct Ps2Driver {
    dual_port: bool,
}

enum Ps2DeviceType {
    AncientAtKeyboard,
    StandardPs2Mouse,
    MouseWithScrollWheel,
    FiveButtonMouse,
    MF2Keyboard,
    ShortKeyboards,
    N97Keyboard,
    _122KeyKeyboard,
    JapaneseGKeyboard,
    JapanesePKeyboard,
    JapaneseAKeyboard,
    NcdSunKeyboard,
}

static PS2_WRITE_LOCK: NoIntSpinlock<Ps2Driver> = NoIntSpinlock::new(Ps2Driver { dual_port: false });

pub static mut PS2_KEYBOARD_STATE: KeyboardState = KeyboardState(0); //so rare we don't care about locking

pub(super) fn init() {
    let mut lock = lock_w_info!(PS2_WRITE_LOCK);
    //disable both devices
    write_command(0xAD);
    write_command(0xA7);
    drain_read_buffer();

    let mut config = get_config_byte();
    println!("PS/2 config byte: {:#010b}", config);
    config &= 0b11111100; //disable interrupts, disable translation
    config |= 0b00110000; //enable both clocks
    println!("PS/2 config byte after modification: {:#010b}", config);
    set_config_byte(config);

    //self test
    write_command(0xAA);
    let self_test_result = read_data();
    if self_test_result != 0x55 {
        panic!("PS/2 self test failed: {:#X}", self_test_result);
    }
    println!("PS/2 self test passed");
    set_config_byte(config); //come ps2 controllers reset during self test

    //enable second port
    write_command(0xA8);
    wait_for_empty_write_buffer();
    let config = get_config_byte();
    let second_port_clock_enabled = config & 0b00100000 == 0;
    lock.dual_port = second_port_clock_enabled;
    println!("PS/2 second port clock enabled: {}", second_port_clock_enabled);
    write_command(0xA7); //disable second port

    //enable first port
    write_command(0xAE);
    if lock.dual_port {
        write_command(0xA8); //enable second port
    }
    let mut config = get_config_byte();
    config |= 0b00000011; //enable interrupts for both ports
    println!("final PS/2 config byte: {:#010b}", config);
    set_config_byte(config);

    reset_device(1);
    if lock.dual_port {
        reset_device(2);
    }

    drop(lock);
}

#[inline(always)]
fn wait_for_empty_write_buffer() {
    while byte_from_port(0x64) & 0x2 != 0 {}
}

#[inline(always)]
fn wait_for_full_read_buffer() {
    while byte_from_port(0x64) & 0x1 == 0 {}
}

#[inline(always)]
fn drain_read_buffer() {
    while byte_from_port(0x64) & 0x1 == 1 {
        read_data();
    }
}

//write lock must be held
fn reset_device(port: u8) {
    send_to_device(port, 0xFF);
    wait_for_full_read_buffer();
    drain_read_buffer();
}

//port is either 1 or 2
//write lock must be held
fn send_to_device(port: u8, byte: u8) {
    if port == 2 {
        write_command(0xD4);
    }
    write_data(byte);
}

#[inline(always)]
fn write_data(byte: u8) {
    wait_for_empty_write_buffer();
    byte_to_port(0x60, byte);
}

#[inline(always)]
fn write_command(byte: u8) {
    wait_for_empty_write_buffer();
    byte_to_port(0x64, byte);
}

#[inline(always)]
fn read_data() -> u8 {
    wait_for_full_read_buffer();
    byte_from_port(0x60)
}

#[inline(always)]
fn try_read_data() -> Option<u8> {
    if byte_from_port(0x64) & 0x1 == 0 {
        None
    } else {
        Some(byte_from_port(0x60))
    }
}

#[inline(always)]
fn read_status() -> u8 {
    byte_from_port(0x64)
}

pub fn reset_cpu() -> ! {
    let lock = lock_w_info!(PS2_WRITE_LOCK);
    write_command(0xFE);
    drop(lock);
    loop {
        //wait for cpu reset
        unsafe { core::arch::asm!("hlt") };
    }
}

//may only be called from interrupt handler
pub fn read_scancodes() -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(byte) = try_read_data() {
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

//write lock must be held
#[inline(always)]
fn get_config_byte() -> u8 {
    write_command(0x20);
    read_data()
}

#[inline(always)]
fn set_config_byte(config: u8) {
    write_command(0x60);
    write_data(config);
}

fn with_devices_disabled<T>(driver: &Ps2Driver, f: impl FnOnce() -> T) -> T {
    write_command(0xAD);
    if driver.dual_port {
        write_command(0xA7);
    }

    drain_read_buffer();

    let result = f();

    write_command(0xAE);
    if driver.dual_port {
        write_command(0xA8);
    }

    result
}

pub fn print_ps2_status() {
    let lock = lock_w_info!(PS2_WRITE_LOCK);
    //disable keyboard

    let (status, config) = with_devices_disabled(&lock, || {
        let status = read_status();
        write_command(0x20);
        let config = read_data();
        (status, config)
    });

    drop(lock);

    println!("PS/2 Status: {:#010b}, Config {:#010b}", status, config);
}
