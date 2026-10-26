//FOR NOW ONLY FULL BYTE COLORS ARE SUPPORTED (32)
#![allow(clippy::identity_op)]

use crate::limine::{self, LIMINE_BOOTLOADER_REQUESTS};
use core::arch::asm;

#[derive(Debug)]
pub struct FrameBuffer {
    pub buffer: *mut u8,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bits_per_pixel: usize,
    pub blue_offset: usize,
    pub green_offset: usize,
    pub red_offset: usize,
    pub blue_size: usize,
    pub green_size: usize,
    pub red_size: usize,
}

#[used]
pub static mut VGA_BINDING: FrameBuffer = FrameBuffer {
    width: 0,
    height: 0,
    stride: 0,
    bits_per_pixel: 0,
    buffer: core::ptr::null_mut(),
    blue_offset: 0,
    green_offset: 0,
    red_offset: 0,
    blue_size: 0,
    green_size: 0,
    red_size: 0,
};

pub fn init_vga_driver() {
    let framebuffer_info = unsafe { &*LIMINE_BOOTLOADER_REQUESTS.frame_buffer_request.info };

    if framebuffer_info.framebuffer_count == 0 {
        panic!("No framebuffers found");
    }

    let framebuffer_slice = unsafe {
        core::slice::from_raw_parts(
            framebuffer_info.framebuffers as *const *const limine::FramebufferInfo,
            framebuffer_info.framebuffer_count as usize,
        )
    };
    let main_framebuffer = unsafe { &*framebuffer_slice[0] };

    //do something with framebuffer modes?

    unsafe {
        VGA_BINDING.width = main_framebuffer.width as usize;
        VGA_BINDING.height = main_framebuffer.height as usize;
        VGA_BINDING.stride = main_framebuffer.pitch as usize;
        VGA_BINDING.bits_per_pixel = main_framebuffer.bpp as usize;
        VGA_BINDING.buffer = main_framebuffer.address as *mut u8;
        VGA_BINDING.blue_offset = main_framebuffer.blue_mask_shift as usize;
        VGA_BINDING.green_offset = main_framebuffer.green_mask_shift as usize;
        VGA_BINDING.red_offset = main_framebuffer.red_mask_shift as usize;
        VGA_BINDING.blue_size = main_framebuffer.blue_mask_size as usize;
        VGA_BINDING.green_size = main_framebuffer.green_mask_size as usize;
        VGA_BINDING.red_size = main_framebuffer.red_mask_size as usize;
    }
    super::vga_text::init_vga_text(main_framebuffer.width as usize, main_framebuffer.height as usize);
}

pub fn clear_screen() {
    unsafe {
        let max_len = VGA_BINDING.stride * VGA_BINDING.height;
        for i in 0..(max_len >> 3) {
            asm!(
                "mov qword ptr [{vga_ptr}], 0x00000000000000000",
                vga_ptr = in(reg) VGA_BINDING.buffer.add(i * 8),
            )
        }
        let max_offset = VGA_BINDING.buffer.add(max_len);
        *max_offset.offset(-8) = 0;
        *max_offset.offset(-7) = 0;
        *max_offset.offset(-6) = 0;
        *max_offset.offset(-5) = 0;
        *max_offset.offset(-4) = 0;
        *max_offset.offset(-3) = 0;
        *max_offset.offset(-2) = 0;
        *max_offset.offset(-1) = 0;
    }
}

pub fn draw_pixel(x: usize, y: usize, color: (u8, u8, u8)) {
    unsafe {
        let offset = (y * VGA_BINDING.stride + x * (VGA_BINDING.bits_per_pixel >> 3)) as isize;
        for pixel_byte in (VGA_BINDING.blue_offset >> 3)..((VGA_BINDING.blue_offset + VGA_BINDING.blue_size) >> 3) {
            *VGA_BINDING.buffer.offset(offset + pixel_byte as isize) = color.0;
        }
        for pixel_byte in (VGA_BINDING.green_offset >> 3)..((VGA_BINDING.green_offset + VGA_BINDING.green_size) >> 3) {
            *VGA_BINDING.buffer.offset(offset + pixel_byte as isize) = color.1;
        }
        for pixel_byte in (VGA_BINDING.red_offset >> 3)..((VGA_BINDING.red_offset + VGA_BINDING.red_size) >> 3) {
            *VGA_BINDING.buffer.offset(offset + pixel_byte as isize) = color.2;
        }
    }
}

pub fn draw_rectangle(x: usize, y: usize, width: usize, height: usize, color: (u8, u8, u8)) {
    for i in 0..width {
        for j in 0..height {
            draw_pixel(x + i, y + j, color);
        }
    }
}

pub fn get_dimensions() -> (usize, usize) {
    unsafe { (VGA_BINDING.width, VGA_BINDING.height) }
}
