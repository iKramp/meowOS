use std::println;

use crate::{
    acpi::cpu_locals::PageFaultHandleMode,
    interrupts::{InterruptProcessorState, disable_interrupts},
    memory,
};

use super::{ProcessData, process_data::CpuStateType, syscall::SyscallCpuState};

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

//this function should NOT use the heap at all to prevent memory leaks by setting IP and SP
pub(super) fn dispatch(new_proc: &ProcessData) -> ! {
    //INFO: any kind of change here should be matched with the one in interrupts/macros.rs and
    //syscall.rs

    let new_page_tree = new_proc.page_tree();
    memory::set_cr3(new_page_tree);
    let mut locals = crate::acpi::cpu_locals::CpuLocals::get_mut();
    disable_interrupts();
    let cpu_state = new_proc.take_cpu_state();
    println!("Dispatching process with state: {:x?}", cpu_state);

    locals.int_depth -= 1;
    locals.lock_info.assert_no_locks();
    locals.page_fault_handle_mode = PageFaultHandleMode::User;
    drop(locals);

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    match cpu_state {
        CpuStateType::Interrupt(interrupt_frame) => return_interrupted(&interrupt_frame),
        CpuStateType::Syscall(state) => return_syscalled(&state),
        CpuStateType::None => panic!("Process with no CPU state dispatched (currently running)"),
    }
}

fn return_interrupted(interrupt_frame: &InterruptProcessorState) -> ! {
    //INFO: any kind of change here should be matched with the one in interrupts/macros.rs

    //make rsp at least return frame size smaller than the start of a page
    let interrupt_frame_addr: u64 = interrupt_frame as *const InterruptProcessorState as u64;
    unsafe {
        core::arch::asm!(
            "mov rsp, {0}",
            "mov r15, [rsp + 8 * 0]",
            "mov r14, [rsp + 8 * 1]",
            "mov r13, [rsp + 8 * 2]",
            "mov r12, [rsp + 8 * 3]",
            "mov r11, [rsp + 8 * 4]",
            "mov r10, [rsp + 8 * 5]",
            "mov r9,  [rsp + 8 * 6]",
            "mov r8,  [rsp + 8 * 7]",
            "mov rbp, [rsp + 8 * 8]",
            "mov rdi, [rsp + 8 * 9]",
            "mov rsi, [rsp + 8 * 10]",
            "mov rdx, [rsp + 8 * 11]",
            "mov rcx, [rsp + 8 * 12]",
            "mov rbx, [rsp + 8 * 13]",
            "mov rax, [rsp + 8 * 14]",
            //rsp + 8 * 15 is error code
            "add rsp, 8 * 16",

            "swapgs", //restore gs for user code

            "iretq",

            in(reg) interrupt_frame_addr
        );
    }
    unreachable!();
}

#[naked]
extern "C" fn return_syscalled(cpu_state: &SyscallCpuState) -> ! {
    //INFO: any kind of change here should be matched with the one in syscall.rs
    unsafe {
        core::arch::naked_asm!(
            //cpu_state in rdi
            // "mov rdx, [rdi + 8 * 0]",
            // "mov rax, [rdi + 8 * 1]",
            // "mov rcx, [rdi + 8 * 2]",
            // "mov r11, [rdi + 8 * 3]",
            // "mov r15, [rdi + 8 * 4]",
            // "mov r14, [rdi + 8 * 5]",
            // "mov r13, [rdi + 8 * 6]",
            // "mov r12, [rdi + 8 * 7]",
            // "mov rbp, [rdi + 8 * 8]",
            // "mov rbx, [rdi + 8 * 9]",
            // "mov rsp, rsi",
            "mov rsi, [rdi + 8 * 1]",
            "mov rbp, [rdi + 8 * 2]",
            "mov rsp, [rdi + 8 * 3]",
            "mov rax, [rdi + 8 * 4]",
            "mov rbx, [rdi + 8 * 5]",
            "mov rcx, [rdi + 8 * 6]",
            "mov rdx, [rdi + 8 * 7]",
            "mov r8,  [rdi + 8 * 8]",
            "mov r9,  [rdi + 8 * 9]",
            "mov r10, [rdi + 8 * 10]",
            "mov r11, [rdi + 8 * 11]",
            "mov r12, [rdi + 8 * 12]",
            "mov r13, [rdi + 8 * 13]",
            "mov r14, [rdi + 8 * 14]",
            "mov r15, [rdi + 8 * 15]",
            "mov rdi, [rdi]",
            "swapgs", //restore gs for user code
            "sysretq",
        )
    }
}
