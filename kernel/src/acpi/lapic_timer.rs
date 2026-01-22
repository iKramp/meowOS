use core::time::Duration;
use std::{println, time::Instant};

use crate::{
    acpi::apic::LapicRegistersPtr, handler, interrupts::{
        TIMER_DESIRED_FREQUENCY, general_interrupt_handler,
        handlers::apic_timer_tick,
        idt::{Entry, IDT},
    }
};

static mut TIMER_CONF: u32 = 0;
static mut FREQUENCY: u64 = 0;

pub(super) fn setup_timer_ap(lapic_registers: &LapicRegistersPtr) {
    unsafe {
        lapic_registers.lvt_timer().bytes().write(TIMER_CONF);
        lapic_registers.divide_configuration().bytes().write(0);
        lapic_registers.initial_count().bytes().write(0); //disable timer
    }
}

pub(super) fn activate_timer(lapic_registers: &LapicRegistersPtr) {
    let mut timer_conf = lapic_registers.lvt_timer().bytes().read();

    timer_conf &= !0xFF_u32;
    timer_conf |= 255; //init the timer vector //TODO reset
    timer_conf &= !(0b11 << 17);
    timer_conf |= 0b00 << 17; //set to oneshot
    timer_conf &= !(1 << 16); //unmask

    const TIMER_COUNT: u32 = u32::MAX;
    lapic_registers.lvt_timer().bytes().write(timer_conf);
    //no division
    lapic_registers.divide_configuration().bytes().write(0b1011);
    lapic_registers.initial_count().bytes().write(TIMER_COUNT);

    let start_time = Instant::now();
    let end_time = start_time + Duration::from_millis(5);
    while Instant::now() < end_time {}

    let ticks = lapic_registers.current_count().bytes().read();
    lapic_registers.initial_count().bytes().write(0); //disable
    crate::interrupts::trigger_pit_eoi();

    let ticks_counted = TIMER_COUNT - ticks;
    let frequency = ticks_counted as u64 * 1_000 / 5; //ticks counted in 5 miliseconds

    println!("Ticks: {}", ticks);

    unsafe { IDT.set(Entry::new(handler!(apic_timer_tick)), 32) };

    let initial_count = ticks_counted * 100 / TIMER_DESIRED_FREQUENCY;
    println!("Initial count: {} or {:x}", initial_count, initial_count);

    timer_conf &= !0xFF_u32;
    timer_conf |= 32; //set correct interrupt vector
    lapic_registers.lvt_timer().bytes().write(timer_conf);
    lapic_registers.initial_count().bytes().write(0);

    unsafe {
        TIMER_CONF = timer_conf;
        FREQUENCY = frequency;
        std::thread::SLEEP = sleep_duration;
    }
}

fn sleep_duration(duration: Duration) {
    let rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "pop {}",
            out(reg) rflags
        );
    }

    let interrupts_enabled = (rflags & (1 << 9)) != 0;
    if interrupts_enabled && duration.as_micros() > 20 {
        set_timeout(duration);
        unsafe { core::arch::asm!("hlt") };
    } else {
        let start = Instant::now();
        while Instant::now() - start < duration {}
    }

}

pub fn set_timeout(duration: Duration) {
    let seconds = duration.as_secs();
    let nanos = duration.subsec_nanos() as u64;
    // divde by | config in division register
    // 2        | 0b0000
    // 4        | 0b0001
    // 8        | 0b0010
    // 16       | 0b0011
    // 32       | 0b1000
    // 64       | 0b1001
    // 128      | 0b1010
    // 1        | 0b1011

    let ticks_seconds = seconds.saturating_mul(unsafe { FREQUENCY });
    let ticks_nanos = nanos.saturating_mul(unsafe { FREQUENCY }) / 1_000_000_000;
    let ticks = ticks_seconds.saturating_add(ticks_nanos);
    let leading_zeros = ticks.leading_zeros();
    let (division, ticks) = match leading_zeros {
        32.. => (0b1011, ticks),        //no division
        31 => (0b0000, ticks / 2),      //divide by 2
        30 => (0b0001, ticks / 4),      //divide by 4
        29 => (0b0010, ticks / 8),      //divide by 8
        28 => (0b0011, ticks / 16),     //divide by 16
        27 => (0b1000, ticks / 32),     //divide by 32
        26 => (0b1001, ticks / 64),     //divide by 64
        25 => (0b1010, ticks / 128),    //divide by 128
        _ => (0b1010, u32::MAX as u64), //more than 10 minutes timeout, treat as max
    };
    let lapic_registers = unsafe { super::LAPIC_REGISTERS.assume_init_mut() };
    lapic_registers.divide_configuration().bytes().write(division);
    lapic_registers.initial_count().bytes().write(ticks as u32);
}
