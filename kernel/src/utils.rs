use core::{arch::asm, ffi::CStr};
use std::{error::KernelError, kerror_unwrapped};

pub fn byte_to_port(port: u16, byte: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") byte);
    }
}

pub fn byte_from_port(port: u16) -> u8 {
    unsafe {
        let byte;
        asm!("in al, dx", in("dx") port, out("al") byte);
        byte
    }
}

pub fn word_to_port(port: u16, word: u16) {
    unsafe {
        asm!("out dx, ax", in("dx") port, in("ax") word);
    }
}

pub fn word_from_port(port: u16) -> u16 {
    unsafe {
        let word;
        asm!("in ax, dx", in("dx") port, out("ax") word);
        word
    }
}

pub fn dword_to_port(port: u16, dword: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") dword);
    }
}

pub fn dword_from_port(port: u16) -> u32 {
    unsafe {
        let dword;
        asm!("in eax, dx", in("dx") port, out("eax") dword);
        dword
    }
}

pub fn ptr_to_str(ptr: *const u8) -> Result<&'static str, KernelError> {
    unsafe {
        let c_str = CStr::from_ptr(ptr as *const i8);
        c_str.to_str().map_err(|_| kerror_unwrapped!(InvalidPointer))
    }
}
