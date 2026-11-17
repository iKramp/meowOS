use core::time::Duration;
use std::{boxed::Box, println, sync::arc::Arc};

use crate::{
    acpi::cpu_locals::CpuLocals,
    arch::{self, memory::VIRTUAL_ADDRESS_BITS},
    clocks::{ScheduledEvent, schedule_event},
    interrupts::{InterruptProcessorState, disable_interrupts, return_interrupted},
    memory::addresses::VirtAddr,
    proc::{MemoryNamespace, signal::SignalData},
};

use super::{ProcessData, process_data::CpuStateType, syscall::return_syscalled};

const MAX_PROC_TIME_SLICE: Duration = Duration::from_millis(100);
const LOG_DISPATCH_STATE: bool = false;

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
pub(super) fn dispatch(new_proc: Arc<ProcessData>) -> ! {
    let cpu_state;
    {
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
        disable_interrupts();
        let event_id = schedule_event(scheduled_event);

        let mut locals = CpuLocals::get_mut();
        cpu_state = new_proc.take_cpu_state();
        if LOG_DISPATCH_STATE {
            println!("Dispatching process with state: {:x?}", cpu_state);
        }

        locals.int_depth -= 1;
        locals.preemtion_id = Some(event_id);

        let dispatch_time = std::time::Instant::now();
        locals.current_process = Some((new_proc, dispatch_time));

        drop(locals);

        unsafe { CpuLocals::get_lock_info().assert_no_locks() };

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }

    match cpu_state {
        CpuStateType::Interrupt(interrupt_frame) => return_interrupted(&interrupt_frame),
        CpuStateType::Syscall(state) => return_syscalled(&state),
        CpuStateType::None => panic!("Process with no CPU state dispatched (currently running)"),
    }
}

pub(super) fn dispatch_signal(new_proc: Arc<ProcessData>, signal_handler: VirtAddr, signal_data: SignalData) {
    let user_context;
    {
        if new_proc.is_in_signal_handler() {
            return;
        }

        let new_page_tree = new_proc.page_tree();
        arch::memory::set_page_tree_root(new_page_tree);

        let scheduled_event = ScheduledEvent {
            time: std::time::Instant::now() + MAX_PROC_TIME_SLICE,
            callback: Box::new(preemtion_callback),
        };
        disable_interrupts();
        let event_id = schedule_event(scheduled_event);

        let mut locals = CpuLocals::get_mut();

        locals.int_depth -= 1;
        locals.preemtion_id = Some(event_id);

        let dispatch_time = std::time::Instant::now();

        user_context = new_proc.take_cpu_state();

        let mutable = new_proc.get_mutable();
        let namespaces = mutable.get_namespaces();
        let mem_namespace = namespaces
            .get_namespace::<MemoryNamespace>(0)
            .expect("Process should always have a memory namespace");
        drop(mutable);
        locals.current_process = Some((new_proc, dispatch_time));
        drop(locals);

        'stack_assertion: {
            let stack_pointer = user_context.get_stack_pointer().expect("Process with no context dispatched");
            let new_stack_pointer = arch::signal::get_new_stack_pointer(stack_pointer, &signal_data);
            let Some(range) = mem_namespace.get_shared_range_from_address(VirtAddr(stack_pointer)) else {
                break 'stack_assertion; //will page fault and will be cleaned up later
            };
            let _ = range.0.expand_to_address(range.1, VirtAddr(new_stack_pointer), 10);
        }

        //make sure pages are mapped

        unsafe { CpuLocals::get_lock_info().assert_no_locks() };

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }

    crate::arch::x86_64::signal::dispatch_signal(user_context, signal_handler, signal_data);
    //catch any return. This is illegal, so crash the program by setting instruction pointer to a
    //non-executable address
    let int_frame = InterruptProcessorState::new(1 << (VIRTUAL_ADDRESS_BITS - 1), 0);
    return_interrupted(&int_frame);
}
