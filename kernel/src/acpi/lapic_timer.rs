use core::time::Duration;
use std::{boxed::Box, println, time::Instant};

use crate::{
    acpi::apic::LapicRegistersPtr,
    handler,
    interrupts::{
        InterruptProcessorState, TIMER_DESIRED_FREQUENCY, disable_interrupts, enable_interrupts,
        handlers::apic_eoi,
        idt::{Entry, IDT},
    },
};

static mut TIMER_CONF: u32 = 0;
static mut FREQUENCY: u64 = 0;
const LAPIC_TIMER_INT_VEC: u8 = 252;

pub struct AcceptedScheduledEvent {
    event: ScheduledEvent,
    id: u64,
}

pub struct ScheduledEvent {
    pub time: Instant,
    pub callback: Box<dyn FnOnce()>,
}

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

    let ticks_counted = TIMER_COUNT - ticks;
    let frequency = ticks_counted as u64 * 1_000 / 5; //ticks counted in 5 miliseconds

    println!("Ticks: {}", ticks);

    unsafe { IDT.set(Entry::new(handler!(apic_interrupt_handler)), LAPIC_TIMER_INT_VEC as usize) };

    let initial_count = ticks_counted * 100 / TIMER_DESIRED_FREQUENCY;
    println!("Initial count: {} or {:x}", initial_count, initial_count);

    timer_conf &= !0xFF_u32;
    timer_conf |= LAPIC_TIMER_INT_VEC as u32; //set correct interrupt vector
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
    let is_root = crate::acpi::cpu_locals::CpuLocals::get().int_depth == 1;
    const ENABLE_LAPIC_SLEEP: bool = false;

    if interrupts_enabled && duration.as_micros() > 20 && is_root && ENABLE_LAPIC_SLEEP {
        set_timeout(duration);
        unsafe { core::arch::asm!("hlt") };
    } else {
        let start = Instant::now();
        while Instant::now() - start < duration {}
    }
}

pub fn schedule_event(event: ScheduledEvent) -> u64 {
    let previous = disable_interrupts();
    let mut locals = crate::acpi::cpu_locals::CpuLocals::get_mut();

    let id = locals.scheduled_event_id_counter;
    locals.scheduled_event_id_counter += 1;

    let event_vec = &mut locals.scheduled_events;

    let event = AcceptedScheduledEvent { event, id };

    let insert_pos = event_vec
        .binary_search_by_key(&event.event.time, |e| e.event.time)
        .unwrap_or_else(|e| e);
    event_vec.insert(insert_pos, event);
    drop(locals);
    handle_scheduled_events();
    if previous {
        enable_interrupts();
    }

    id
}

pub fn cancel_scheduled_event(id: u64) -> bool {
    let previous = disable_interrupts();
    let mut locals = crate::acpi::cpu_locals::CpuLocals::get_mut();

    let event_vec = &mut locals.scheduled_events;

    if let Some(pos) = event_vec.iter().position(|e| e.id == id) {
        event_vec.remove(pos);
        drop(locals);
        handle_scheduled_events();
        if previous {
            enable_interrupts();
        }
        true
    } else {
        drop(locals);
        if previous {
            enable_interrupts();
        }
        false
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

pub extern "C" fn apic_interrupt_handler(_proc_data: &mut InterruptProcessorState) {
    handle_scheduled_events();
    apic_eoi();
}

pub fn handle_scheduled_events() {
    let mut now = Instant::now();

    let previous = disable_interrupts();

    let mut locals = crate::acpi::cpu_locals::CpuLocals::get_mut();
    while let Some(event) = locals.scheduled_events.first() {
        if event.event.time > now {
            break;
        }
        let event = locals.scheduled_events.remove(0);
        (event.event.callback)();
        now = Instant::now();
    }

    if let Some(next_event) = locals.scheduled_events.first() {
        let time_until_next = next_event.event.time - now;
        set_timeout(time_until_next);
    }

    drop(locals);

    if previous {
        enable_interrupts();
    }
}
