use core::time::Duration;
use std::{boxed::Box, println};

use crate::{
    acpi::cpu_locals::CpuLocals,
    arch,
    clocks::{ScheduledEvent, schedule_event},
    interrupts::{disable_interrupts, return_interrupted},
};

use super::{ProcessData, process_data::CpuStateType, syscall::return_syscalled};

const MAX_PROC_TIME_SLICE: Duration = Duration::from_millis(100);

/*
 * Things that need to be done: (Intel SDM, Vol 3, chapter 8.1.2
 * Keep segment registers CS, DS, SS, ES, FS, Gs the same (do nothing)
 * Push general purpose registers. After this, they can be modified again to aid in saving the rest
 * of the state
 * Push E/RFLAGS
 * Push RIP
 * Push CR3
 * Update CPU locals to indicate a process being run?
 * Save fpu, mmx... state with fxsave64. Enable REX.W
 * save/restore gs and fs registers  through MSRs and swapgs
 */

fn preemtion_callback() {
    //nothing, when an interrupt triggers a new process will be scheduled automatically
}

//WARN: DROP ALL HEAP DATA BEFORE DISPATCHING to avoid leaking memory
pub(super) fn dispatch(new_proc: &ProcessData) -> ! {
    //INFO: any kind of change here should be matched with the one in interrupts/macros.rs and
    //syscall.rs

    //change page tree
    let new_page_tree = new_proc.page_tree();
    arch::memory::set_page_tree_root(new_page_tree);

    //schedule preemption
    let scheduled_event = ScheduledEvent {
        time: std::time::Instant::now() + MAX_PROC_TIME_SLICE,
        callback: Box::new(preemtion_callback),
    };
    let event_id = schedule_event(scheduled_event);

    disable_interrupts();

    let mut locals = CpuLocals::get_mut();
    let cpu_state = new_proc.take_cpu_state();
    println!("Dispatching process with state: {:x?}", cpu_state);

    locals.int_depth -= 1;
    locals.preemtion_id = Some(event_id);
    drop(locals);

    unsafe { CpuLocals::get_lock_info().assert_no_locks() };

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    match cpu_state {
        CpuStateType::Interrupt(interrupt_frame) => return_interrupted(&interrupt_frame),
        CpuStateType::Syscall(state) => return_syscalled(&state),
        CpuStateType::None => panic!("Process with no CPU state dispatched (currently running)"),
    }
}
