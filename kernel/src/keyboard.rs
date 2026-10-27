use std::vec::Vec;

use crate::drivers::input::evnet_codes::*;

bitfield::bitfield! {
    pub struct KeyboardState(u16);
    impl Debug;
    pub lshift, set_lshift: 0;
    pub rshift, set_rshift: 1;
    pub fakelshift, set_fakelshift: 2;
    pub fakershift, set_fakershift: 3;
    pub lctrl, set_lctrl: 4;
    pub rctrl, set_rctrl: 5;
    pub lalt, set_lalt: 6;
    pub ralt, set_ralt: 7;
    pub lsuper, set_lsuper: 8;
    pub rsuper, set_rsuper: 9;

    //add lock keys
}

impl KeyboardState {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn shift(&self) -> bool {
        (self.lshift() && self.fakelshift()) || (self.rshift() && self.fakershift())
    }

    pub fn ctrl(&self) -> bool {
        self.lctrl() || self.rctrl()
    }

    pub fn alt(&self) -> bool {
        self.lalt() || self.ralt()
    }

    pub fn super_key(&self) -> bool {
        self.lsuper() || self.rsuper()
    }
}

pub fn handle_keyboard_data(bytes: Vec<u8>, state: &mut KeyboardState) -> Vec<(Key, KeyEvent)> {
    let mut bytes_slice = bytes.as_slice();
    let mut events = Vec::new();
    while !bytes_slice.is_empty() {
        let byte = bytes_slice[0];
        let new_event;

        if [0x00, 0xee, 0xf1, 0xfa, 0xfc, 0xfd, 0xfe, 0xff].contains(&byte) {
            //aa is also a
            //protocol code, but
            //is needed for shift
            bytes_slice = &bytes_slice[1..];
            continue; //ignore these scancodes for now, protocol scancodes
        }

        if [0xe0, 0xe1, 0xe2].contains(&byte) {
            let (key_event_opt, seq_len) = Key::from_sequence(bytes_slice);
            bytes_slice = &bytes_slice[seq_len..];
            if let Some((key, key_event)) = key_event_opt {
                new_event = (key, key_event);
            } else {
                continue; //unknown sequence, ignore
            }
        } else {
            let key_event = if byte & 0x80 == 0 {
                KeyEvent::Pressed
            } else {
                KeyEvent::Released
            };

            let key_code = byte & 0x7F;
            if let Some(key) = Key::from_single_byte(key_code) {
                new_event = (key, key_event);
                bytes_slice = &bytes_slice[1..];
            } else {
                continue; //unknown key, ignore
            }
        }

        match new_event.0 {
            Key::LShift => {
                state.set_lshift(new_event.1 == KeyEvent::Pressed);
                state.set_fakelshift(new_event.1 == KeyEvent::Pressed);
            }
            Key::RShift => {
                state.set_rshift(new_event.1 == KeyEvent::Pressed);
                state.set_fakershift(new_event.1 == KeyEvent::Pressed);
            }
            Key::FakeLShift => {
                state.set_fakelshift(new_event.1 == KeyEvent::Pressed);
            }
            Key::FakeRshift => {
                state.set_fakershift(new_event.1 == KeyEvent::Pressed);
            }
            Key::Lctrl => {
                state.set_lctrl(new_event.1 == KeyEvent::Pressed);
            }
            Key::Rctrl => {
                state.set_rctrl(new_event.1 == KeyEvent::Pressed);
            }
            Key::LAlt => {
                state.set_lalt(new_event.1 == KeyEvent::Pressed);
            }
            Key::RAlt => {
                state.set_ralt(new_event.1 == KeyEvent::Pressed);
            }
            Key::LSuper => {
                state.set_lsuper(new_event.1 == KeyEvent::Pressed);
            }
            Key::RSuper => {
                state.set_rsuper(new_event.1 == KeyEvent::Pressed);
            }
            _ => {}
        }

        events.push(new_event);
    }

    events
}

#[derive(Debug, PartialEq, Eq)]
pub enum KeyEvent {
    Pressed,
    Released,
}

impl From<KeyEvent> for u16 {
    fn from(value: KeyEvent) -> Self {
        match value {
            KeyEvent::Pressed => 1,
            KeyEvent::Released => 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Key {
    Esc, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    Backtick, K1, K2, K3, K4, K5, K6, K7, K8, K9, K0, Dash, Equal, Backspace,
    Tab, Q, W, E, R, T, Y, U, I, O, P, LeftBracket, RightBracket, Enter,
    CapsLock, A, S, D, F, G, H, J, K, L, Semicolon, Quote, Backslash,
    LShift, Z, X, C, V, B, N, M, Comma, Dot, Slash, RShift,
    Lctrl, LSuper, LAlt, Space, RAlt, RSuper, Menu, Rctrl,
    LeftArrow, UpArrow, RightArrow, DownArrow,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    NumDot, NumSlash, NumAsterisk, NumMinus, NumPlus, NumEnter,
    NumLock, ScrollLock, PrtScn,
    CtrlBreak, Home, Up, PgUp, Left, Right, End, Down, PgDn, Insert, Delete,
    FakeLShift, FakeRshift
}

impl Key {
    pub fn from_single_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => None,
            0x01 => Some(Key::Esc),
            0x02 => Some(Key::K1),
            0x03 => Some(Key::K2),
            0x04 => Some(Key::K3),
            0x05 => Some(Key::K4),
            0x06 => Some(Key::K5),
            0x07 => Some(Key::K6),
            0x08 => Some(Key::K7),
            0x09 => Some(Key::K8),
            0x0A => Some(Key::K9),
            0x0B => Some(Key::K0),
            0x0C => Some(Key::Dash),
            0x0D => Some(Key::Equal),
            0x0E => Some(Key::Backspace),
            0x0F => Some(Key::Tab),
            0x10 => Some(Key::Q),
            0x11 => Some(Key::W),
            0x12 => Some(Key::E),
            0x13 => Some(Key::R),
            0x14 => Some(Key::T),
            0x15 => Some(Key::Y),
            0x16 => Some(Key::U),
            0x17 => Some(Key::I),
            0x18 => Some(Key::O),
            0x19 => Some(Key::P),
            0x1A => Some(Key::LeftBracket),
            0x1B => Some(Key::RightBracket),
            0x1C => Some(Key::Enter),
            0x1D => Some(Key::Lctrl),
            0x1E => Some(Key::A),
            0x1F => Some(Key::S),
            0x20 => Some(Key::D),
            0x21 => Some(Key::F),
            0x22 => Some(Key::G),
            0x23 => Some(Key::H),
            0x24 => Some(Key::J),
            0x25 => Some(Key::K),
            0x26 => Some(Key::L),
            0x27 => Some(Key::Semicolon),
            0x28 => Some(Key::Quote),
            0x29 => Some(Key::Backtick),
            0x2A => Some(Key::LShift),
            0x2B => Some(Key::Backslash),
            0x2C => Some(Key::Z),
            0x2D => Some(Key::X),
            0x2E => Some(Key::C),
            0x2F => Some(Key::V),
            0x30 => Some(Key::B),
            0x31 => Some(Key::N),
            0x32 => Some(Key::M),
            0x33 => Some(Key::Comma),
            0x34 => Some(Key::Dot),
            0x35 => Some(Key::Slash),
            0x36 => Some(Key::RShift),
            0x37 => Some(Key::NumAsterisk),
            0x38 => Some(Key::LAlt),
            0x39 => Some(Key::Space),
            0x3A => Some(Key::CapsLock),
            0x3B => Some(Key::F1),
            0x3C => Some(Key::F2),
            0x3D => Some(Key::F3),
            0x3E => Some(Key::F4),
            0x3F => Some(Key::F5),
            0x40 => Some(Key::F6),
            0x41 => Some(Key::F7),
            0x42 => Some(Key::F8),
            0x43 => Some(Key::F9),
            0x44 => Some(Key::F10),
            0x45 => Some(Key::NumLock),
            0x46 => Some(Key::ScrollLock),
            0x47 => Some(Key::Num7),
            0x48 => Some(Key::Num8),
            0x49 => Some(Key::Num9),
            0x4A => Some(Key::NumMinus),
            0x4B => Some(Key::Num4),
            0x4C => Some(Key::Num5),
            0x4D => Some(Key::Num6),
            0x4E => Some(Key::NumPlus),
            0x4F => Some(Key::Num1),
            0x50 => Some(Key::Num2),
            0x51 => Some(Key::Num3),
            0x52 => Some(Key::Num0),
            0x53 => Some(Key::NumDot),
            0x57 => Some(Key::F11),
            0x58 => Some(Key::F12),
            _ => None,
        }
    }

    fn from_sequence(bytes: &[u8]) -> (Option<(Self, KeyEvent)>, usize) {
        if bytes.is_empty() {
            return (None, 0);
        }
        match bytes[0] {
            0xE0 => {
                if bytes.len() < 2 {
                    return (None, 1); //should be invalid anyway, but just skip E0
                }

                let key_event = if bytes[1] & 0x80 == 0 {
                    KeyEvent::Pressed
                } else {
                    KeyEvent::Released
                };

                match bytes[1] & 0x7F {
                    0x1C => (Some((Key::NumEnter, key_event)), 2),
                    0x1D => (Some((Key::Rctrl, key_event)), 2),
                    0x2A => (Some((Key::FakeLShift, key_event)), 2),
                    0x35 => (Some((Key::NumSlash, key_event)), 2),
                    0x36 => (Some((Key::FakeRshift, key_event)), 2),
                    0x37 => (Some((Key::PrtScn, key_event)), 2),
                    0x38 => (Some((Key::RAlt, key_event)), 2),
                    0x47 => (Some((Key::Home, key_event)), 2),
                    0x48 => (Some((Key::UpArrow, key_event)), 2),
                    0x49 => (Some((Key::PgUp, key_event)), 2),
                    0x4B => (Some((Key::LeftArrow, key_event)), 2),
                    0x4D => (Some((Key::RightArrow, key_event)), 2),
                    0x4F => (Some((Key::End, key_event)), 2),
                    0x50 => (Some((Key::DownArrow, key_event)), 2),
                    0x51 => (Some((Key::PgDn, key_event)), 2),
                    0x52 => (Some((Key::Insert, key_event)), 2),
                    0x53 => (Some((Key::Delete, key_event)), 2),
                    0x5B => (Some((Key::LSuper, key_event)), 2),
                    0x5C => (Some((Key::RSuper, key_event)), 2),
                    0x5D => (Some((Key::Menu, key_event)), 2),
                    _ => (None, 2), //unknown E0 sequence, skip it
                }
            }
            0xE1 => {
                (None, 3) //ignore this whole weird sequence
            }
            _ => (None, 1),
        }
    }
}

impl From<Key> for EventCode {
    fn from(value: Key) -> Self {
        match value {
            Key::Esc => KEY_ESC,
            Key::F1 => KEY_F1,
            Key::F2 => KEY_F2,
            Key::F3 => KEY_F3,
            Key::F4 => KEY_F4,
            Key::F5 => KEY_F5,
            Key::F6 => KEY_F6,
            Key::F7 => KEY_F7,
            Key::F8 => KEY_F8,
            Key::F9 => KEY_F9,
            Key::F10 => KEY_F10,
            Key::F11 => KEY_F11,
            Key::F12 => KEY_F12,
            Key::F13 => KEY_F13,
            Key::F14 => KEY_F14,
            Key::F15 => KEY_F15,
            Key::F16 => KEY_F16,
            Key::F17 => KEY_F17,
            Key::F18 => KEY_F18,
            Key::F19 => KEY_F19,
            Key::F20 => KEY_F20,
            Key::F21 => KEY_F21,
            Key::F22 => KEY_F22,
            Key::F23 => KEY_F23,
            Key::F24 => KEY_F24,
            Key::Backtick => KEY_GRAVE,
            Key::K1 => KEY_1,
            Key::K2 => KEY_2,
            Key::K3 => KEY_3,
            Key::K4 => KEY_4,
            Key::K5 => KEY_5,
            Key::K6 => KEY_6,
            Key::K7 => KEY_7,
            Key::K8 => KEY_8,
            Key::K9 => KEY_9,
            Key::K0 => KEY_0,
            Key::Dash => KEY_MINUS,
            Key::Equal => KEY_EQUAL,
            Key::Backspace => KEY_BACKSPACE,
            Key::Tab => KEY_TAB,
            Key::Q => KEY_Q,
            Key::W => KEY_W,
            Key::E => KEY_E,
            Key::R => KEY_R,
            Key::T => KEY_T,
            Key::Y => KEY_Y,
            Key::U => KEY_U,
            Key::I => KEY_I,
            Key::O => KEY_O,
            Key::P => KEY_P,
            Key::LeftBracket => KEY_LEFTBRACE,
            Key::RightBracket => KEY_RIGHTBRACE,
            Key::Enter => KEY_ENTER,
            Key::CapsLock => KEY_CAPSLOCK,
            Key::A => KEY_A,
            Key::S => KEY_S,
            Key::D => KEY_D,
            Key::F => KEY_F,
            Key::G => KEY_G,
            Key::H => KEY_H,
            Key::J => KEY_J,
            Key::K => KEY_K,
            Key::L => KEY_L,
            Key::Semicolon => KEY_SEMICOLON,
            Key::Quote => KEY_APOSTROPHE,
            Key::Backslash => KEY_BACKSLASH,
            Key::LShift => KEY_LEFTSHIFT,
            Key::Z => KEY_Z,
            Key::X => KEY_X,
            Key::C => KEY_C,
            Key::V => KEY_V,
            Key::B => KEY_B,
            Key::N => KEY_N,
            Key::M => KEY_M,
            Key::Comma => KEY_COMMA,
            Key::Dot => KEY_DOT,
            Key::Slash => KEY_SLASH,
            Key::RShift => KEY_RIGHTSHIFT,
            Key::Lctrl => KEY_LEFTCTRL,
            Key::LSuper => KEY_LEFTMETA,
            Key::LAlt => KEY_LEFTALT,
            Key::Space => KEY_SPACE,
            Key::RAlt => KEY_RIGHTALT,
            Key::RSuper => KEY_RIGHTMETA,
            Key::Menu => KEY_MENU,
            Key::Rctrl => KEY_RIGHTCTRL,
            Key::LeftArrow => KEY_LEFT,
            Key::UpArrow => KEY_UP,
            Key::RightArrow => KEY_RIGHT,
            Key::DownArrow => KEY_DOWN,
            Key::Num0 => KEY_KP0,
            Key::Num1 => KEY_KP1,
            Key::Num2 => KEY_KP2,
            Key::Num3 => KEY_KP3,
            Key::Num4 => KEY_KP4,
            Key::Num5 => KEY_KP5,
            Key::Num6 => KEY_KP6,
            Key::Num7 => KEY_KP7,
            Key::Num8 => KEY_KP8,
            Key::Num9 => KEY_KP9,
            Key::NumDot => KEY_KPDOT,
            Key::NumSlash => KEY_KPSLASH,
            Key::NumAsterisk => KEY_KPASTERISK,
            Key::NumMinus => KEY_KPMINUS,
            Key::NumPlus => KEY_KPPLUS,
            Key::NumEnter => KEY_KPENTER,
            Key::NumLock => KEY_NUMLOCK,
            Key::ScrollLock => KEY_SCROLLLOCK,
            Key::PrtScn => KEY_SYSRQ,
            Key::FakeLShift => KEY_RESERVED, //not passed on
            Key::FakeRshift => KEY_RESERVED,
            Key::CtrlBreak => KEY_PAUSE,
            Key::Home => KEY_HOME,
            Key::Up => KEY_UP,
            Key::PgUp => KEY_PAGEUP,
            Key::Left => KEY_LEFT,
            Key::Right => KEY_RIGHT,
            Key::End => KEY_END,
            Key::Down => KEY_DOWN,
            Key::PgDn => KEY_PAGEDOWN,
            Key::Insert => KEY_INSERT,
            Key::Delete => KEY_DELETE,
        }
    }
}
