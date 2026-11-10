use std::lock_w_info;

use crate::{
    acpi::cpu_locals::CpuLocals,
    task_runner::{self, has_tasks_to_process},
};

use super::{PROC_INITIALIZED, SCHEDULER, dispatcher::dispatch};

pub extern "C" fn interrupt_context_switch() {
    context_switch();
}

pub fn context_switch() {
    let locals = CpuLocals::get();
    if !unsafe { PROC_INITIALIZED } || !locals.proc_initialized {
        return;
    }

    #[cfg(debug_assertions)] //interrupts should already check this
    if locals.int_depth != 1 {
        panic!("Invalid context switch state: int_depth = {}", locals.int_depth);
    }
    drop(locals);

    no_ret_context_switch();
}

pub fn no_ret_context_switch() -> ! {
    // Switch to the next process
    loop {
        task_runner::process_tasks();
        task_runner::run_repeating_tasks();

        let mut scheduler_lock = lock_w_info!(SCHEDULER);
        let scheduler = unsafe { scheduler_lock.assume_init_mut() };
        if let Some(process_data_arc) = scheduler.schedule() {
            drop(scheduler_lock);
            dispatch(process_data_arc)
        }
        drop(scheduler_lock);
        //wait here
        if !has_tasks_to_process() {
            std::thread::sleep(core::time::Duration::from_millis(20));
        }
    }
}
