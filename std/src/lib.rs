#![allow(incomplete_features)]
#![no_std]
#![feature(ptr_metadata)]
#![feature(specialization)]
#![feature(negative_impls)]
#![feature(coerce_unsized)]
#![feature(unsize)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        let mut lock = crate::print::PRINT.as_mut().unwrap().force_get_lock();
        print::_print_colored_locked((0, 0, 255), &mut lock, format_args!("{}", info), print::LogLevel::Error);
        loop {
            crate::thread::sleep(crate::time::Duration::from_secs(10));
        }
    }
}

extern crate alloc;

pub mod print;
pub use print::Print;
pub use print::set_print;

pub mod double_map;
pub mod error;
pub mod ffi_future;
pub mod mem_utils;
pub mod queue;

pub mod cow;
pub use core::any;
pub use core::arch;
pub use core::array;
pub use core::ascii;
use core::panic::PanicInfo;
//backtrace
pub use alloc::boxed;
pub use alloc::boxed::*;
pub use alloc::collections;
pub use core::borrow;
pub use core::cell;
pub use core::char;
pub use core::clone;
pub use core::cmp;
pub use core::convert;
pub use core::default;
pub use core::env;
//error
pub use core::f32;
pub use core::f64;
pub use core::ffi;
pub use core::fmt;
//fs
pub use core::future;
pub use core::hash;
pub use core::hint;
//
//i8   depracation planned
//i16  depracation planned
//i32  depracation planned
//i64  depracation planned
//i128 depracation planned
//io
//isize depracation planned
pub use core::iter;
pub use core::marker;
pub use core::mem;
pub use core::net;
pub use core::num;
pub use core::ops;
pub use core::option;
//os
pub mod eh;
pub use eh::panic;
//path
pub use core::pin;
pub use core::prelude;
pub use core::primitive;
//
//process
pub use core::ptr;
pub mod rc;
pub use alloc::string;
pub use alloc::string::String;
pub use core::result;
pub use core::slice;
pub use core::str;
pub mod sync;
pub use core::task;
pub mod thread;
pub mod time;
//u8   depracation planned
//u16  depracation planned
//u32  depracation planned
//u64  depracation planned
//u128 depracation planned
//usize depracation planned
pub use alloc::vec;
pub use alloc::vec::Vec;
//assert_matches experimental
//async_iter     experimental
//intrinsics     experimental
//simd           experimental
