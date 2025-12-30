use std::arch::asm;
use std::{lock_w_info, print, println, printlnc};

use crate::utils::byte_to_port;

use super::font::*;
use super::vga_driver::VGA_BINDING;
use std::sync::no_int_spinlock::*;

#[derive(Debug)]
pub struct VgaText {
    pub foreground: (u8, u8, u8),
    pub background: (u8, u8, u8),
    height_lines: usize,
    width_chars: usize,
    line: usize,
    char: usize,
}

impl VgaText {
    pub fn write_text(&mut self, text: &str) {
        for char in text.as_bytes() {
            byte_to_port(0xe9, *char);
            if char == &b'\n' {
                self.do_newline();
            } else {
                unsafe {
                    self.write_character(*char);
                }
            }
        }
    }

    fn do_newline(&mut self) {
        self.char = 0;
        self.line += 1;
        if self.line >= self.height_lines - 1 {
            self.line -= 1;
            self.scroll();
        }
    }

    fn up_line(&mut self) {
        if self.line > 0 {
            self.line -= 1;
        }
    }

    pub unsafe fn write_character(&mut self, character: u8) {
        if !(0x20..0x7f).contains(&character) {
            // character = &0xfe
            let high_nibble = character >> 4;
            let low_nibble = character & 0b1111;
            fn transform_nibble(character: u8) -> u8 {
                if character < 10 { b'0' + character } else { b'A' + character }
            }

            let high_char = transform_nibble(high_nibble);
            let low_char = transform_nibble(low_nibble);
            unsafe {
                self.write_character(b'\\');
                self.write_character(b'x');
                self.write_character(high_char);
                self.write_character(low_char);
            }
        }


        let character = &DEFAULT_FONT[character as usize * 8..(character as usize + 1) * 8];
        let mut curr_row = self.line * CHAR_HEIGHT;
        for char_line in character {
            for i in 0..8 {
                let bit = char_line & (128 >> i) != 0;
                let color = match bit {
                    true => self.foreground,
                    false => self.background,
                };
                crate::vga::vga_driver::draw_pixel(self.char * CHAR_WIDTH + i, curr_row, color);
            }
            curr_row += 1;
        }

        self.char += 1;
        if self.char >= self.width_chars {
            self.do_newline();
        }
    }

    fn scroll(&mut self) {
        unsafe {
            let top_ptr = VGA_BINDING.buffer;
            let diff = VGA_BINDING.stride * CHAR_HEIGHT;
            let limit = top_ptr.add(diff * (self.height_lines - 1) - 1);

            asm!(
                "2:",
                "mov r8, [r10 + r9]",
                "mov qword ptr [r10], r8",
                "add r10, 8",
                "cmp r10, r11",
                "jle 2b",
                in("r9") diff,
                in("r10") top_ptr,
                in("r11") limit,
                lateout("r9") _,
                lateout("r10") _,
                lateout("r11") _,
                out("r8") _,
            )
        }
    }
}

impl core::fmt::Write for VgaText {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_text(s);
        Ok(())
    }
}

impl std::Print for VgaText {
    fn set_bg_color(&mut self, color: (u8, u8, u8)) {
        self.background = color;
    }

    fn set_fg_color(&mut self, color: (u8, u8, u8)) {
        self.foreground = color;
    }

    fn reset_color(&mut self) {
        self.foreground = (255, 255, 255);
        self.background = (0, 0, 0);
    }
}

#[used]
pub static mut VGA_TEXT: NoIntSpinlock<VgaText> = NoIntSpinlock::new(VgaText {
    background: (0, 0, 0),
    foreground: (255, 255, 255),
    height_lines: 0,
    width_chars: 0,
    line: 0,
    char: 0,
});

pub fn init_vga_text(width: usize, height: usize) {
    unsafe {
        let mut display = lock_w_info!(VGA_TEXT);
        display.height_lines = height / (CHAR_HEIGHT);
        display.width_chars = width / (CHAR_WIDTH);
        std::set_print(core::ptr::addr_of_mut!(VGA_TEXT));
    }
}

pub fn clear_screen() {
    super::vga_driver::clear_screen();
    unsafe {
        let mut display = lock_w_info!(VGA_TEXT);
        display.line = 0;
        display.char = 0;
    }
    let vga_buffer_addr = unsafe { VGA_BINDING.buffer };
    println!("VGA buffer address: {:#x?}", vga_buffer_addr);
}

pub fn hello_message() {
    print!("Hello via ");

    printlnc!(
        (30, 105, 210),
        "RustOS\n
            .  :*. ## .*:  .            \n    
         :  @@*@@@@@@@@@@*@@  :         \n
        :@@@@@@@@@@  @@@@@@@@@@:        \n
      @@@@@@#+:   =%%=   :+#@@@@@@      \n
   :*+@@@@+.                .+@@@@+*:   \n
   .@@@@@@@@@@@@@@@@@@@@@%#+.  +@@@@.   \n
 .%@@@@@@@@@@@@@@@@@@@@@@@@@@=  +@@@@%. \n
 :+@#.=@*-*@@@@@@-----=#@@@@@@ :@=.#@+: \n
.*@@@##+. =@@@@@@------*@@@@@* .+##@@@*.\n
:+@@@+    =@@@@@@@@@@@@@@@@%-     +@@@+:\n
:+@@@+    =@@@@@@####%@@@@@@*    =#@@@+:\n
.*@@@#    =@@@@@@     -@@@@@@+  =@@@@@*.\n
 :+@@@@@@@@@@@@@@@@@-  *@@@@@@@@@@@@@+: \n
 .%@@@@@@@@@@@@@@@@@-  .@@@@@@@@@@@@@%. \n
   .@@@@%==++-------.    --++==%@@@@.   \n
   :*+@@@@*+@=            =@+*@@@@+*:   \n
      @@@@=-@%:          :%@-=@@@@      \n 
        :@@@@@@@@%####%@@@@@@@@:        \n
         :  @@*@@@@@@@@@@*@@  :         \n
            .  :*. ## .*:  .            \n

             "
    );
}
