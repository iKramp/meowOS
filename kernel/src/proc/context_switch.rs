use std::lock_w_info;

use crate::{
    acpi::cpu_locals::CpuLocals,
    proc::dispatcher::dispatch_signal,
    task_runner::{self, has_tasks_to_process},
};

use super::{PROC_INITIALIZED, SCHEDULER, dispatcher::dispatch};

pub extern "C" fn interrupt_context_switch() {
    context_switch();
}

pub fn context_switch() {
    {
        let locals = CpuLocals::get();
        if !unsafe { PROC_INITIALIZED } || !locals.proc_initialized {
            return;
        }

        #[cfg(debug_assertions)] //interrupts should already check this
        if locals.int_depth != 1 {
            panic!("Invalid context switch state: int_depth = {}", locals.int_depth);
        }
    }

    no_ret_context_switch();
}

pub fn no_ret_context_switch() -> ! {
    loop {
        // Switch to the next process
        let proc_data_arc = loop {
            task_runner::process_tasks();
            task_runner::run_repeating_tasks();

            let mut scheduler_lock = lock_w_info!(SCHEDULER);
            let scheduler = unsafe { scheduler_lock.assume_init_mut() };
            if let Some(process_data_arc) = scheduler.schedule() {
                break process_data_arc;
            }

            //TODO: get processes with pending signals
            drop(scheduler_lock);
            //wait here
            if !has_tasks_to_process() {
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        };

        let to_handle = proc_data_arc
            .get_mutable()
            .get_signal_state_mut()
            .get_to_handle(&proc_data_arc);

        if let Some(signal_to_handle) = to_handle {
            if proc_data_arc.is_killed() {
                debug_assert!(
                    false,
                    "process with pid {:?} has pending signal {:?} while being killed. This should not happen",
                    proc_data_arc.pid(),
                    signal_to_handle.0
                );
                continue;
            }
            dispatch_signal(proc_data_arc, signal_to_handle.0, signal_to_handle.1);
        } else {
            if proc_data_arc.is_killed() {
                continue;
            }
            dispatch(proc_data_arc);
        }
    }
}
