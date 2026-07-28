use core::mem::MaybeUninit;
use std::{boxed::Box, println, time::Instant};

use crate::{
    clocks::{hpet::HpetWrapper, tsc::TscWrapper},
    handler, interrupts,
};

mod hpet;
mod rtc;
mod tsc;

static mut SELECTED_TIMER: MaybeUninit<Box<dyn Timer>> = MaybeUninit::uninit();
const TIMER_INTERRUPT_VECTOR: usize = 251;

trait Timer {
    fn init(&mut self) -> bool;
    fn get_time(&self) -> Instant;
    fn calibrate(&mut self, current: Instant);
    fn service_interrupt(&self) {}
}

pub fn init() {
    let timers: [Box<dyn Timer>; 2] = [Box::new(TscWrapper::new()), Box::new(HpetWrapper::new())];
    for mut timer in timers {
        if try_use_timer(&mut timer) {
            unsafe {
                SELECTED_TIMER = MaybeUninit::new(timer);
            }
            unsafe {
                interrupts::idt::IDT.set(
                    interrupts::idt::Entry::new(handler!(service_interrupt)),
                    TIMER_INTERRUPT_VECTOR,
                );
            }
            unsafe {
                std::time::GET_TIME = || SELECTED_TIMER.assume_init_ref().get_time();
            }

            return;
        }
    }

    panic!("No suitable timer found");
}

fn try_use_timer(timer: &mut Box<dyn Timer>) -> bool {
    let success = timer.init();
    if success {
        let now = rtc::RTC_WRAPPER.get_time();
        timer.calibrate(now);
        println!(level:info, "Current time: {:?}", timer.get_time());
    }
    success
}

fn service_interrupt() {
    unsafe { SELECTED_TIMER.assume_init_ref().service_interrupt() };
}
