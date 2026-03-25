use core::ptr::addr_of;
use std::{Print, lock_w_info, print::LogLevel, sync::no_int_spinlock::NoIntSpinlock};

use crate::{
    utils::byte_to_port,
    vga::vga_text::{VGA_TEXT, VgaText},
};

static PRINT: NoIntSpinlock<Printer> = NoIntSpinlock::new(Printer::new(&VGA_TEXT));

pub fn init_printer() {
    unsafe { std::set_print(addr_of!(PRINT)) };
}

struct Printer {
    vga_text: &'static NoIntSpinlock<VgaText>,
    log_level: LogLevel,
}

impl Printer {
    pub const fn new(vga_text: &'static NoIntSpinlock<VgaText>) -> Self {
        Self {
            vga_text,
            log_level: LogLevel::Info,
        }
    }

    pub fn init(&self) {
        unsafe { std::set_print(core::ptr::addr_of!(PRINT)) };
    }
}

fn num_to_chars(num: u8) -> [u8; 3] {
    let hundreds = num / 100;
    let tens = (num % 100) / 10;
    let units = num % 10;
    [hundreds + b'0', tens + b'0', units + b'0']
}

impl Print for Printer {
    fn set_bg_color(&mut self, color: (u8, u8, u8)) {
        let mut vga = lock_w_info!(self.vga_text);
        vga.set_bg_color(color);
        // for byte in b"\x1b[48;2;" {
        //     byte_to_port(0xe9, *byte);
        // }
        // for byte in &num_to_chars(color.2) {
        //     byte_to_port(0xe9, *byte);
        // }
        // byte_to_port(0xe9, b';');
        // for byte in &num_to_chars(color.1) {
        //     byte_to_port(0xe9, *byte);
        // }
        // byte_to_port(0xe9, b';');
        // for byte in &num_to_chars(color.0) {
        //     byte_to_port(0xe9, *byte);
        // }
        // byte_to_port(0xe9, b'm');
    }

    fn set_fg_color(&mut self, color: (u8, u8, u8)) {
        let mut vga = lock_w_info!(self.vga_text);
        vga.set_fg_color(color);
        // for byte in b"\x1b[38;2;" {
        //     byte_to_port(0xe9, *byte);
        // }
        // for byte in &num_to_chars(color.2) {
        //     byte_to_port(0xe9, *byte);
        // }
        // byte_to_port(0xe9, b';');
        // for byte in &num_to_chars(color.1) {
        //     byte_to_port(0xe9, *byte);
        // }
        // byte_to_port(0xe9, b';');
        // for byte in &num_to_chars(color.0) {
        //     byte_to_port(0xe9, *byte);
        // }
        // byte_to_port(0xe9, b'm');
    }

    fn reset_color(&mut self) {
        let mut vga = lock_w_info!(self.vga_text);
        vga.reset_color();
        // byte_to_port(0xe9, 0x1b); //reset terminal color
        // byte_to_port(0xe9, b'[');
        // byte_to_port(0xe9, b'0');
        // byte_to_port(0xe9, b'm');
    }

    fn set_log_level(&mut self, log_level: std::print::LogLevel) {
        self.log_level = log_level;
    }
}

impl core::fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.log_level != LogLevel::Debug {
            let mut vga = lock_w_info!(self.vga_text);
            let _ = vga.write_str(s); //ignore errors to vga, serial works either way
        }

        for char in s.as_bytes() {
            byte_to_port(0xe9, *char);
        }
        Ok(())
    }
}
