//ps/2 mouse

use crate::drivers::{
    input::{self, InputDeviceType},
    ps2::Ps2DeviceType,
};

pub(super) fn handle_mouse_input(scancodes: &[u8], _dev_type: Ps2DeviceType) {
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
                code: crate::drivers::input::evnet_codes::REL_X,
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
