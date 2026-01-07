use core::ptr::addr_of;
use std::{Print, format, lock_w_info, sync::no_int_spinlock::NoIntSpinlock};

use crate::{
    utils::byte_to_port,
    vga::vga_text::{VGA_TEXT, VgaText},
};

static PRINT: NoIntSpinlock<Printer> = NoIntSpinlock::new(Printer::new(&VGA_TEXT));

pub fn init_printer() {
    unsafe { std::set_print(addr_of!(PRINT)) };
}

enum PrintTarget {
    Vga,
    E9Port,
    Both,
}

struct Printer {
    vga_text: &'static NoIntSpinlock<VgaText>,
    target: PrintTarget,
}

impl Printer {
    pub const fn new(vga_text: &'static NoIntSpinlock<VgaText>) -> Self {
        Self { vga_text, target: PrintTarget::Both }
    }

    pub fn init(&self) {
        unsafe { std::set_print(core::ptr::addr_of!(PRINT)) };
    }
}

fn num_to_chars(num: u8) -> [u8; 3] {
    let hundreds = num / 100;
    let tens = (num % 100) / 10;
    let units = num % 10;
    [
        hundreds + b'0',
        tens + b'0',
        units + b'0',
    ]
}

impl Print for Printer {
    fn set_bg_color(&mut self, color: (u8, u8, u8)) {
        let mut vga = lock_w_info!(self.vga_text);
        vga.set_bg_color(color);
        for byte in b"\x1b[48;2;" {
            byte_to_port(0xe9, *byte);
        }
        for byte in &num_to_chars(color.2) {
            byte_to_port(0xe9, *byte);
        }
        byte_to_port(0xe9, b';');
        for byte in &num_to_chars(color.1) {
            byte_to_port(0xe9, *byte);
        }
        byte_to_port(0xe9, b';');
        for byte in &num_to_chars(color.0) {
            byte_to_port(0xe9, *byte);
        }
        byte_to_port(0xe9, b'm');
    }

    fn set_fg_color(&mut self, color: (u8, u8, u8)) {
        let mut vga = lock_w_info!(self.vga_text);
        vga.set_fg_color(color);
        for byte in b"\x1b[38;2;" {
            byte_to_port(0xe9, *byte);
        }
        for byte in &num_to_chars(color.2) {
            byte_to_port(0xe9, *byte);
        }
        byte_to_port(0xe9, b';');
        for byte in &num_to_chars(color.1) {
            byte_to_port(0xe9, *byte);
        }
        byte_to_port(0xe9, b';');
        for byte in &num_to_chars(color.0) {
            byte_to_port(0xe9, *byte);
        }
        byte_to_port(0xe9, b'm');
    }

    fn reset_color(&mut self) {
        let mut vga = lock_w_info!(self.vga_text);
        vga.reset_color();
        byte_to_port(0xe9, 0x1b); //reset terminal color
        byte_to_port(0xe9, b'[');
        byte_to_port(0xe9, b'0');
        byte_to_port(0xe9, b'm');
    }
}

impl core::fmt::Write for Printer {
    fn write_str(&mut self, mut s: &str) -> core::fmt::Result {
        if s.starts_with("@VGA") {
            self.target = PrintTarget::Vga;
            if s.len() <= 5 {
                return Ok(());
            }
            s = &s[5..];
        }
        if s.starts_with("@DBG") {
            self.target = PrintTarget::E9Port;
            if s.len() <= 5 {
                return Ok(());
            }
            s = &s[5..];
        }
        if s.starts_with("@BOTH") {
            self.target = PrintTarget::Both;
            if s.len() <= 6 {
                return Ok(());
            }
            s = &s[6..];
        }
        match self.target {
            PrintTarget::Vga => {
                let mut vga = lock_w_info!(self.vga_text);
                vga.write_str(s)?
            }
            PrintTarget::E9Port => {
                for char in s.as_bytes() {
                    byte_to_port(0xe9, *char);
                }
            }
            PrintTarget::Both => {
                for char in s.as_bytes() {
                    byte_to_port(0xe9, *char);
                }
                let mut vga = lock_w_info!(self.vga_text);
                vga.write_str(s)?
            }
        }
        Ok(())
    }
}
