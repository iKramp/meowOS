use crate::acpi::cpu_locals::CpuLocals;
use crate::arch::interrupts as arch_interrupts;
use crate::proc::StackCpuStateData;
use crate::proc::interrupt_context_switch;
use crate::proc::release_current_proc;
use crate::proc::save_cpu_state;

pub use arch_interrupts::InterruptProcessorState;
pub use arch_interrupts::return_interrupted;

pub type InterruptHandler = extern "C" fn(&mut InterruptProcessorState) -> InterruptReturnType;

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
    proc_data: &mut InterruptProcessorState,
    atomic_int: u64,
    main_handler: InterruptHandler,
) {
    let mut locals = CpuLocals::get_mut();
    let prev_atomic = locals.atomic_context;
    locals.int_depth += 1;
    locals.atomic_context |= atomic_int != 0;
    let atomic_context = locals.atomic_context;
    drop(locals);

    if !atomic_context {
        enable_interrupts();
    }

    let return_type = main_handler(proc_data);

    disable_interrupts();

    //proc is depth 0, root int is depth 1

    let mut locals = CpuLocals::get_mut();
    let root_int = !(locals.int_depth > 1 || locals.atomic_context);

    //for now we always reschedule if we're root int so ForceReschedule is not needed. Use if
    //sometimes we want to return to proc instead of rescheduling

    let reschedule = root_int; //use return_type == InterruptReturnType::ForceReschedule;
    if reschedule {
        if let Some(curr_proc) = locals.current_process.clone() {
            //save current process state
            save_cpu_state(&StackCpuStateData::Interrupt(proc_data), &curr_proc);
            //can't hold locals through this call
            drop(locals);
            release_current_proc(&curr_proc);
        } else {
            drop(locals);
        }

        interrupt_context_switch();

        if return_type == InterruptReturnType::ForceReschedule {
            panic!("Process was killed during interrupt handling, but no reschedule was performed");
        }

        locals = CpuLocals::get_mut();
    }

    locals.int_depth -= 1;
    locals.atomic_context = prev_atomic;
    drop(locals);

    unsafe { CpuLocals::get_lock_info().assert_no_locks() };

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
