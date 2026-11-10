use std::sync::arc::Arc;
use std::time::Instant;
use std::time::UsDuration;

use crate::acpi::cpu_locals::CpuLocals;
use crate::arch::interrupts as arch_interrupts;
use crate::proc::ProcessData;
use crate::proc::StackCpuStateData;
use crate::proc::interrupt_context_switch;
use crate::proc::release_current_proc;
use crate::proc::save_cpu_state;

pub use arch_interrupts::InterruptProcessorState;
pub use arch_interrupts::return_interrupted;

pub type InterruptHandler = extern "C" fn(&mut InterruptProcessorState, Option<&Arc<ProcessData>>) -> InterruptReturnType;

#[inline(always)]
pub fn enable_interrupts() {
    arch_interrupts::enable_interrupts();
}

///Returns true if interrupts were enabled before this was called
#[inline(always)]
pub fn disable_interrupts() -> bool {
    arch_interrupts::disable_interrupts()
}

pub fn init_interrupts() {
    arch_interrupts::init_interrupts();
}

pub fn end_of_interrupt() {
    arch_interrupts::end_of_interrupt();
}

pub fn register_interrupt_handler(handler: extern "C" fn() -> !, interrupt_index: u64) {
    arch_interrupts::register_interrupt_handler(handler, interrupt_index);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptReturnType {
    Normal,
    ForceReschedule, //if proc was killed
}

pub extern "C" fn general_interrupt_handler(
    cpu_data: &mut InterruptProcessorState,
    atomic_int: u64,
    main_handler: InterruptHandler,
) {
    if atomic_int != 0 {
        atomic_interrupt_handler(cpu_data, main_handler);
        return;
    }
    let int_entry_time = Instant::now();

    let mut locals = CpuLocals::get_mut();
    locals.int_depth += 1;
    let curr_proc = locals.current_process.take();
    drop(locals);
    enable_interrupts();

    if let Some((curr_proc, dispatch_time)) = &curr_proc {
        let cpu_time = int_entry_time.duration_since(*dispatch_time);
        let cpu_time_micros = cpu_time.as_micros() as i64;
        let cpu_time_us_duration = UsDuration::from_micros(cpu_time_micros);
        curr_proc.add_cpu_time_userspace(cpu_time_us_duration);
    }

    let return_type = main_handler(cpu_data, curr_proc.as_ref().map(|(proc, _)| proc));

    disable_interrupts();
    //proc is depth 0, root int is depth 1

    //interrupts have to be disabled before get_mut so when
    //locals is dropped at the end, they aren't re-enabled
    let mut locals = CpuLocals::get_mut();
    let root_int = locals.int_depth <= 1;

    //for now we always reschedule if we're root int so ForceReschedule is not needed. Use if
    //sometimes we want to return to proc instead of rescheduling

    let reschedule = root_int; //use return_type == InterruptReturnType::ForceReschedule;
    if reschedule {
        drop(locals);
        //re-enable interrupts for interrupt_context_switch. It needs
        //interrupts for when the kernel can't dispatch a process and just waits
        //for an event
        enable_interrupts();

        if let Some((curr_proc, _)) = curr_proc {
            //save current process state
            save_cpu_state(&StackCpuStateData::Interrupt(cpu_data), &curr_proc);
            //can't hold locals through this call
            release_current_proc(&curr_proc);
        }

        //extremely likely to not return
        interrupt_context_switch();

        if return_type == InterruptReturnType::ForceReschedule {
            panic!("Process was killed during interrupt handling, but no reschedule was performed");
        }

        disable_interrupts();
        locals = CpuLocals::get_mut();
    }

    locals.int_depth -= 1;
    let int_exit_time = Instant::now();
    let int_time = int_exit_time.duration_since(int_entry_time);
    locals.int_time += int_time;

    drop(locals);

    unsafe { CpuLocals::get_lock_info().assert_no_locks() };

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

fn atomic_interrupt_handler(proc_data: &mut InterruptProcessorState, main_handler: InterruptHandler) {
    //don't touch any cpu locals. Handlers of atomic interrupts should also not touch locals

    main_handler(proc_data, None);
    unsafe { CpuLocals::get_lock_info().assert_no_locks() };
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
