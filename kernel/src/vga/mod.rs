mod font;
pub mod vga_driver;
pub mod vga_text;

pub use vga_text::clear_screen;

pub use vga_driver::init_vga_driver;

pub use vga_driver::print_framebuffer_info;
