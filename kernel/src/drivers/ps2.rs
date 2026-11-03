use std::{lock_w_info, println, sync::no_int_spinlock::NoIntSpinlock, vec::Vec};

use crate::{
    drivers::{
        input::{self, InputDeviceType, evnet_codes::REL_X},
        keyboard,
    },
    utils::{byte_from_port, byte_to_port},
};

use super::keyboard::KeyboardState;

struct Ps2Driver {
    dual_port: bool,
    port_1_device: Option<Ps2DeviceType>,
    port_2_device: Option<Ps2DeviceType>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Ps2DeviceType {
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

impl Ps2DeviceType {
    pub fn is_keyboard(&self) -> bool {
        matches!(
            self,
            Ps2DeviceType::AncientAtKeyboard
                | Ps2DeviceType::MF2Keyboard
                | Ps2DeviceType::ShortKeyboards
                | Ps2DeviceType::N97Keyboard
                | Ps2DeviceType::_122KeyKeyboard
                | Ps2DeviceType::JapaneseGKeyboard
                | Ps2DeviceType::JapanesePKeyboard
                | Ps2DeviceType::JapaneseAKeyboard
        )
    }

    pub fn is_mouse(&self) -> bool {
        matches!(
            self,
            Ps2DeviceType::StandardPs2Mouse | Ps2DeviceType::MouseWithScrollWheel | Ps2DeviceType::FiveButtonMouse
        )
    }
}

static PS2_WRITE_LOCK: NoIntSpinlock<Ps2Driver> = NoIntSpinlock::new(Ps2Driver {
    dual_port: false,
    port_1_device: None,
    port_2_device: None,
});

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

    lock.port_1_device = get_identification(1);
    if lock.dual_port {
        lock.port_2_device = get_identification(2);
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
//data, port
fn read_data() -> u8 {
    wait_for_full_read_buffer();
    let data = byte_from_port(0x60);
    data
}

#[inline(always)]
//data, port
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
//(port 1, port 2)
pub fn read_scancodes() -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(byte) = try_read_data() {
        bytes.push(byte);
    }
    bytes
}

pub fn handle_ps2_interrupt(port: u8) {
    let lock = lock_w_info!(PS2_WRITE_LOCK);
    let scancodes = crate::drivers::ps2::read_scancodes();
    if let Some(device) = lock.port_1_device
        && port == 1
    {
        if device.is_keyboard() {
            handle_ps2_keyboard_scancodes(scancodes);
        } else if device.is_mouse() {
            handle_ps2_mouse_scancodes(scancodes, device);
        }
    } else if let Some(device) = lock.port_2_device
        && port == 2
    {
        if device.is_keyboard() {
            handle_ps2_keyboard_scancodes(scancodes);
        } else if device.is_mouse() {
            handle_ps2_mouse_scancodes(scancodes, device);
        }
    }
    drop(lock);
}

fn handle_ps2_keyboard_scancodes(scancodes: Vec<u8>) {
    let keeb_state = unsafe { &mut PS2_KEYBOARD_STATE };
    let keys = keyboard::handle_keyboard_data(scancodes, keeb_state);
    if !input::INPUT_EVENT_MANAGER.buffers_exist() {
        crate::tty::handle_input(keys.into_boxed_slice(), keeb_state);
    } else {
        let events = keys
            .into_iter()
            .map(|k| crate::drivers::input::InputEvent {
                generated_at: std::time::Instant::now().into(),
                device_type: (&InputDeviceType::Keyboard).into(),
                code: u16::from(k.0),
                value: k.1.into(),
            })
            .collect::<Vec<_>>();
        input::INPUT_EVENT_MANAGER.add_events(Some(InputDeviceType::Keyboard), None, &events);
    }
}

fn handle_ps2_mouse_scancodes(scancodes: Vec<u8>, dev_type: Ps2DeviceType) {
    static mut MOUSE_BUTTON_STATE: (bool, bool, bool) = (false, false, false); //left, right, middle
    for bytes in scancodes.chunks_exact(3) {
        let x = bytes[1] as u16 as i16; //extend without sign extension
        let y = bytes[2] as u16 as i16; //extend without sign extension
        let rel_x = x - if bytes[0] & 0x10 != 0 { 256 } else { 0 };
        let rel_y = y - if bytes[0] & 0x20 != 0 { 256 } else { 0 };
        let buttons = bytes[0] & 0b00000111;
        let left_button = buttons & 0b00000001 != 0;
        let right_button = buttons & 0b00000010 != 0;
        let middle_button = buttons & 0b00000100 != 0;
        if rel_x != 0 {
            let event = crate::drivers::input::InputEvent {
                generated_at: std::time::Instant::now().into(),
                device_type: (&InputDeviceType::Mouse).into(),
                code: REL_X,
                value: rel_x.cast_unsigned(),
            };
            input::INPUT_EVENT_MANAGER.add_events(Some(InputDeviceType::Mouse), None, &[event]);
        }
        if rel_y != 0 {
            let event = crate::drivers::input::InputEvent {
                generated_at: std::time::Instant::now().into(),
                device_type: (&InputDeviceType::Mouse).into(),
                code: crate::drivers::input::evnet_codes::REL_Y,
                value: rel_y.cast_unsigned(),
            };
            input::INPUT_EVENT_MANAGER.add_events(Some(InputDeviceType::Mouse), None, &[event]);
        }
        if left_button != unsafe { MOUSE_BUTTON_STATE.0 } {
            let event = crate::drivers::input::InputEvent {
                generated_at: std::time::Instant::now().into(),
                device_type: (&InputDeviceType::Mouse).into(),
                code: crate::drivers::input::evnet_codes::BTN_LEFT,
                value: if left_button { 1 } else { 0 },
            };
            input::INPUT_EVENT_MANAGER.add_events(Some(InputDeviceType::Mouse), None, &[event]);
        }
        if right_button != unsafe { MOUSE_BUTTON_STATE.1 } {
            let event = crate::drivers::input::InputEvent {
                generated_at: std::time::Instant::now().into(),
                device_type: (&InputDeviceType::Mouse).into(),
                code: crate::drivers::input::evnet_codes::BTN_RIGHT,
                value: if right_button { 1 } else { 0 },
            };
            input::INPUT_EVENT_MANAGER.add_events(Some(InputDeviceType::Mouse), None, &[event]);
        }
        if middle_button != unsafe { MOUSE_BUTTON_STATE.2 } {
            let event = crate::drivers::input::InputEvent {
                generated_at: std::time::Instant::now().into(),
                device_type: (&InputDeviceType::Mouse).into(),
                code: crate::drivers::input::evnet_codes::BTN_MIDDLE,
                value: if middle_button { 1 } else { 0 },
            };
            input::INPUT_EVENT_MANAGER.add_events(Some(InputDeviceType::Mouse), None, &[event]);
        }
        unsafe {
            MOUSE_BUTTON_STATE = (left_button, right_button, middle_button);
        }
    }
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

//returns false on timeout
fn set_enable_scanning(port: u8, enable: bool) -> bool {
    let cmd = if enable { 0xF4 } else { 0xF5 };
    send_to_device(port, cmd);
    let now = std::time::Instant::now();
    while now.elapsed().as_millis() < 10 {
        let data = try_read_data();
        if let Some(0xFA) = data {
            return true;
        } else if let Some(data) = data {
            println!("data {}", data);
        }
    }
    false
}

fn get_identification(port: u8) -> Option<Ps2DeviceType> {
    drain_read_buffer();
    let ack1 = set_enable_scanning(1, false);
    let ack2 = set_enable_scanning(2, false);
    if !ack1 || !ack2 {
        println!(level:warn, "devices did not ack disable scanning");
    }
    drain_read_buffer();

    send_to_device(port, 0xF2); //get device ID
    let now = std::time::Instant::now();
    let mut got_ack = false;
    while now.elapsed().as_millis() < 100 {
        let data = try_read_data();
        if let Some(0xFA) = data {
            got_ack = true;
            break;
        }
    }
    if !got_ack {
        panic!("PS/2 device did not respond to get device ID command on port {}", port);
    }
    let mut id_data = [0_u8; 2];
    let mut id_index = 0;
    let now = std::time::Instant::now();
    while now.elapsed().as_millis() < 10 {
        let data = try_read_data();
        if let Some(byte) = data {
            id_data[id_index] = byte;
            id_index += 1;
            if id_index >= 2 {
                break;
            }
        }
    }

    let device_type;

    if id_index == 0 {
        device_type = Some(Ps2DeviceType::AncientAtKeyboard);
    } else if id_index == 1 {
        match id_data[0] {
            0x00 => device_type = Some(Ps2DeviceType::StandardPs2Mouse),
            0x03 => device_type = Some(Ps2DeviceType::MouseWithScrollWheel),
            0x04 => device_type = Some(Ps2DeviceType::FiveButtonMouse),
            _ => device_type = None,
        }
    } else {
        match (id_data[0], id_data[1]) {
            (0xAB, 0x83) => device_type = Some(Ps2DeviceType::MF2Keyboard),
            (0xAB, 0x41) => device_type = Some(Ps2DeviceType::MF2Keyboard),
            (0xAB, 0xC1) => device_type = Some(Ps2DeviceType::MF2Keyboard),
            (0xAB, 0x84) => device_type = Some(Ps2DeviceType::ShortKeyboards),
            (0xAB, 0x54) => device_type = Some(Ps2DeviceType::ShortKeyboards),
            (0xAB, 0x85) => device_type = Some(Ps2DeviceType::N97Keyboard),
            (0xAB, 0x86) => device_type = Some(Ps2DeviceType::_122KeyKeyboard),
            (0xAB, 0x90) => device_type = Some(Ps2DeviceType::JapaneseGKeyboard),
            (0xAB, 0x91) => device_type = Some(Ps2DeviceType::JapanesePKeyboard),
            (0xAB, 0x92) => device_type = Some(Ps2DeviceType::JapaneseAKeyboard),
            (0xAC, 0xA1) => device_type = Some(Ps2DeviceType::NcdSunKeyboard),
            _ => device_type = None,
        }
    }

    set_enable_scanning(1, true);
    set_enable_scanning(2, true);

    device_type
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
        let config = get_config_byte();
        (status, config)
    });

    drop(lock);

    println!("PS/2 Status: {:#010b}, Config {:#010b}", status, config);
}
