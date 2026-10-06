use crate::utils::byte_to_port;

const PIC1: u16 = 0x20;
const PIC2: u16 = 0xA0; /* IO base address for slave PIC */
const PIC1_COMMAND: u16 = PIC1;
const PIC1_DATA: u16 = PIC1 + 1;
const PIC2_COMMAND: u16 = PIC2;
const PIC2_DATA: u16 = PIC2 + 1;

pub static mut APIC_TIMER_INIT: bool = false;
pub const PIC_TIMER_ORIGINAL_FREQ: u32 = 1_193_182;

pub(super) fn init_pic() {
    byte_to_port(PIC1_COMMAND, 0x11);
    byte_to_port(PIC2_COMMAND, 0x11);

    byte_to_port(PIC1_DATA, 0x20);
    byte_to_port(PIC2_DATA, 0x28);

    byte_to_port(PIC1_DATA, 0x04);
    byte_to_port(PIC2_DATA, 0x02);

    byte_to_port(PIC1_DATA, 0x01);
    byte_to_port(PIC2_DATA, 0x01);

    disable_timer();

    byte_to_port(PIC1_DATA, 0xFE); //only allow timer
    byte_to_port(PIC2_DATA, 0xFE);
}

fn disable_pic_keep_timer() {
    byte_to_port(PIC1_DATA, 0xFE); //mask interrupts, keep timer
    byte_to_port(PIC2_DATA, 0xFE);

    byte_to_port(PIC1_DATA - 1, 0x20); //trigger EOI
    byte_to_port(PIC2_DATA - 1, 0x20);
}

pub(in crate::arch::x86_64) fn disable_pic_completely() {
    byte_to_port(PIC1_DATA, 0xFF); //mask interrupts
    byte_to_port(PIC2_DATA, 0xFF);

    byte_to_port(PIC1_DATA - 1, 0x20); //trigger EOI
    byte_to_port(PIC2_DATA - 1, 0x20);

    disable_timer();

    disconnect_imcr();
}

fn disconnect_imcr() {
    const IMCR: u16 = 0x22;

    byte_to_port(IMCR, 0x70);
    byte_to_port(IMCR + 1, 0x01);
}

fn disable_timer() {
    #[allow(clippy::unusual_byte_groupings)]
    byte_to_port(0x43, 0b00_11_000_0);
}

///Max 50 miliseconds
pub(in crate::arch::x86_64) fn set_pit_timeout(timeout_nanoseconds: u32) {
    let divisor = PIC_TIMER_ORIGINAL_FREQ as u64 * timeout_nanoseconds as u64 / 1_000_000_000;
    let divisor_low = (divisor & 0xFF) as u8;
    let divisor_high = ((divisor >> 8) & 0xFF) as u8;

    //one-shot mode
    #[allow(clippy::unusual_byte_groupings)]
    byte_to_port(0x43, 0b00_11_000_0); // set mode to one-shot
    byte_to_port(0x40, divisor_low); // set low byte
    byte_to_port(0x40, divisor_high); // set high byte
}

pub fn trigger_pit_eoi() {
    // Trigger EOI for PIT
    byte_to_port(PIC1_DATA - 1, 0x20);
    byte_to_port(PIC2_DATA - 1, 0x20);
}
