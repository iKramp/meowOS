use std::lock_w_info;

use crate::{acpi::cpu_locals::CpuLocals, task_runner};

use super::{PROC_INITIALIZED, ProcessData, SCHEDULER, dispatcher::dispatch};

pub extern "C" fn interrupt_context_switch() {
    context_switch();
}

pub fn context_switch() {
    let locals = CpuLocals::get();
    if !unsafe { PROC_INITIALIZED } || !locals.proc_initialized {
        return;
    }

    #[cfg(debug_assertions)] //interrupts should already check this
    if locals.int_depth != 1 || locals.atomic_context {
        panic!(
            "Invalid context switch state: int_depth = {}, atomic_context = {}",
            locals.int_depth, locals.atomic_context
        );
    }

    no_ret_context_switch();
}

pub fn no_ret_context_switch() -> ! {
    let cpu_locals = CpuLocals::get();

    // Switch to the next process
    loop {
        task_runner::process_tasks();

        let mut scheduler_lock = lock_w_info!(SCHEDULER);
        let scheduler = unsafe { scheduler_lock.assume_init_mut() };
        if let Some(process_data_arc) = scheduler.schedule() {
            cpu_locals.current_process = Some(process_data_arc.clone());
            let process_data_ptr = process_data_arc.get() as *const ProcessData;
            drop(process_data_arc);
            let process_data = unsafe { &*process_data_ptr }; //safe because it's saved in cpu locals
            drop(scheduler_lock);
            dispatch(process_data)
        }
        drop(scheduler_lock);
        //wait here
        std::thread::sleep(core::time::Duration::from_millis(20));
    }
}
