use core::fmt::{Arguments, Write};

use crate::{lock_w_info, sync::no_int_spinlock::*};

pub static mut PRINT: Option<&NoIntSpinlock<dyn Print>> = None;

///# Safety
///printer must be a valid pointer
pub unsafe fn set_print(printer: *const NoIntSpinlock<dyn Print>) {
    unsafe { PRINT = Some(&*printer) }
}

pub trait Print: Write {
    fn set_bg_color(&mut self, color: (u8, u8, u8));
    fn set_fg_color(&mut self, color: (u8, u8, u8));
    fn reset_color(&mut self);
    fn set_log_level(&mut self, log_level: LogLevel);
    fn print(&mut self, args: core::fmt::Arguments) {
        let res = self.write_fmt(args).is_ok();
        if !res {
            self.set_fg_color((0, 0, 255));
            let _ = self.write_str("[print error]");
            self.reset_color();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[macro_export]
macro_rules! format_location_print {
    ($($arg:tt)*) => (format_args!("[{}:{}]: {}", file!(), line!(), format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    (level:$lvl:ident, $($arg:tt)*) => ($crate::print::_print($crate::format_location_print!($($arg)*), $crate::convert_level!($lvl)));
    ($($arg:tt)*) => ($crate::print::_print($crate::format_location_print!($($arg)*), $crate::convert_level!(default_log_level)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    (level:$lvl:ident, $($arg:tt)*) => ($crate::print!(level:$lvl, "{}\n", format_args!($($arg)*)));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! printc {
    (level:$lvl:ident, $fg:expr, $($arg:tt)*) => ($crate::print::_print_colored($fg, $crate::format_location_print!($($arg)*), $crate::convert_level!($lvl)));
    ($fg:expr, $($arg:tt)*) => ($crate::print::_print_colored($fg, $crate::format_location_print!($($arg)*), $crate::convert_level!(default_log_level)));
}

#[macro_export]
macro_rules! printlnc {
    (level:$lvl:ident, $fg:expr, $($arg:tt)*) => ($crate::printc!(level:$lvl, $fg, "{}\n", format_args!($($arg)*)));
    ($fg:expr, $($arg:tt)*) => ($crate::printc!($fg, "{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! convert_level {
    (error) => {
        $crate::print::LogLevel::Error
    };
    (warn) => {
        $crate::print::LogLevel::Warn
    };
    (info) => {
        $crate::print::LogLevel::Info
    };
    (debug) => {
        $crate::print::LogLevel::Debug
    };
    (default_log_level) => {
        $crate::print::LogLevel::Debug
    };
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments, log_level: LogLevel) {
    let mut lock = unsafe { lock_w_info!(PRINT.as_mut().expect("printer was not set before printing")) };
    _print_locked(&mut lock, args, log_level);
}

#[doc(hidden)]
pub fn _print_locked(lock: &mut NoIntSpinlockGuard<dyn Print>, args: core::fmt::Arguments, log_level: LogLevel) {
    lock.set_log_level(log_level);
    lock.print(args);
}

#[doc(hidden)]
pub fn _print_colored(fg: (u8, u8, u8), args: core::fmt::Arguments, log_level: LogLevel) {
    let mut lock = unsafe { lock_w_info!(PRINT.as_mut().expect("printer was not set before printing")) };
    lock.set_fg_color(fg);
    _print_locked(&mut lock, args, log_level);
    lock.reset_color();
}

#[doc(hidden)]
pub fn _print_colored_locked(
    fg: (u8, u8, u8),
    lock: &mut NoIntSpinlockGuard<dyn Print>,
    args: core::fmt::Arguments,
    log_level: LogLevel,
) {
    lock.set_fg_color(fg);
    _print_locked(lock, args, log_level);
    lock.reset_color();
}

#[must_use]
#[inline]
pub fn _format(args: Arguments<'_>) -> crate::String {
    fn format_inner(args: Arguments<'_>) -> crate::String {
        let mut output = crate::String::new();
        let res = output.write_fmt(args);
        if res.is_err() {
            output.push_str("[format error]");
        }
        output
    }

    args.as_str()
        .map_or_else(|| format_inner(args), crate::alloc::borrow::ToOwned::to_owned)
}

#[macro_export]
macro_rules! format {
    ($($arg:tt)*) => ($crate::print::_format(core::format_args!($($arg)*)));
}
