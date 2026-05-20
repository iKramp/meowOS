use std::println;

use crate::{
    acpi::cpu_locals::{CpuLocals, PageFaultHandleMode},
    proc::{StackCpuStateData, interrupt_context_switch, release_current_proc, save_cpu_state},
};

use super::{disable_interrupts, enable_interrupts};

#[macro_export]
macro_rules! handler {
    (
        $name:ident $(, $flag:ident )* $(,)?
    ) => {{
        use $crate::interrupts::general_interrupt_handler;
        #[naked]
        extern "C" fn wrapper() -> ! {
            //TODO: any kind of change here should be matched with the one in dispatcher.rs
            unsafe { core::arch::naked_asm!(
                //potentially padding, but we always start with a clean stack if we're the first
                //level. IS aligned to 16 bytes
                //
                //pre-pushed values
                //
                //ss (64 bit) //rsp + 21 * 8
                //rsp //rsp + 20 * 8
                //rflags //rsp + 19 * 8
                //cs (64 bit) //rsp + 18 * 8
                //rip //rsp + 17 * 8
                //
                //possibly error code rsp + 16 * 8

                handler!(@if_else_flag has_code, $($flag)*, {
                    "
                        sub rsp, 16 * 8
                    "
                } {
                    "
                        sub rsp, 17 * 8
                        mov qword ptr [rsp + 16 * 8], 0
                    "
                }),

                //save all general purpose registers
                "mov qword ptr [rsp + 15 * 8], rax",
                "mov qword ptr [rsp + 14 * 8], rbx",
                "mov qword ptr [rsp + 13 * 8], rcx",
                "mov qword ptr [rsp + 12 * 8], rdx",
                "mov qword ptr [rsp + 11 * 8], rsi",
                "mov qword ptr [rsp + 10 * 8], rdi",
                "mov qword ptr [rsp + 9 * 8], rbp",
                //not pushing stack pointer, that's already changed
                "mov qword ptr [rsp + 8 * 8], r8",
                "mov qword ptr [rsp + 7 * 8], r9",
                "mov qword ptr [rsp + 6 * 8], r10",
                "mov qword ptr [rsp + 5 * 8], r11",
                "mov qword ptr [rsp + 4 * 8], r12",
                "mov qword ptr [rsp + 3 * 8], r13",
                "mov qword ptr [rsp + 2 * 8], r14",
                "mov qword ptr [rsp + 1 * 8], r15",


                handler!(@if_else_flag slow_swap, $($flag)*, {
                    "
                        mov ebx, 1 //idk some flag linux sets
                        mov ecx, 0xc0000101 //gs_base
                        rdmsr
                        test edx, edx
                        js 3f

                        swapgs
                        mov qword ptr [rsp], 0x1 //swapped
                        jmp 4f
                        3:
                        mov qword ptr [rsp], 0x0 //not swapped
                        4:
                    "
                } {
                    //mov pushed cs to register by offsetting from rsp
                    "
                        xor rax, rax
                        mov rax, qword ptr [rsp + 8 * 18] //cs
                                                          //skip swap if rax == 8
                        cmp rax, 8
                        je 3f

                        swapgs
                        mov qword ptr [rsp], 0x1 //swapped
                        jmp 4f
                        3:
                        mov qword ptr [rsp], 0x0 //not swapped
                        4:
                    "
                }),

                "mov rdi, rsp",
                "add rdi, 8", //start of proc data

                handler!(@if_else_flag slow_swap, $($flag)*, {
                    "mov rsi, 1 //atomic interrupt"
                } {
                    "mov rsi, 0 //not atomic interrupt"
                }),

                "lea rdx, [rip + {0}]", //main handler
                //stack should be aligned

                "call {1}",

                "mov rax, qword ptr [rsp]", //check if we swapped
                "cmp rax, 0",
                "je 5f",

                "swapgs",
                "5:",

                "mov r15, qword ptr [rsp + 1 * 8]",
                "mov r14, qword ptr [rsp + 2 * 8]",
                "mov r13, qword ptr [rsp + 3 * 8]",
                "mov r12, qword ptr [rsp + 4 * 8]",
                "mov r11, qword ptr [rsp + 5 * 8]",
                "mov r10, qword ptr [rsp + 6 * 8]",
                "mov r9,  qword ptr [rsp + 7 * 8]",
                "mov r8,  qword ptr [rsp + 8 * 8]",
                "mov rbp, qword ptr [rsp + 9 * 8]",
                "mov rdi, qword ptr [rsp + 10 * 8]",
                "mov rsi, qword ptr [rsp + 11 * 8]",
                "mov rdx, qword ptr [rsp + 12 * 8]",
                "mov rcx, qword ptr [rsp + 13 * 8]",
                "mov rbx, qword ptr [rsp + 14 * 8]",
                "mov rax, qword ptr [rsp + 15 * 8]",

                //restore stack
                "add rsp, 17 * 8",

                "iretq",
                sym $name,
                sym general_interrupt_handler,
            )}
        }
        wrapper
    }};

    (@if_flag $target:ident, $($flag:ident)* { $($true:tt)* }) => {
        handler!(@match_flag $target, $($flag)*, {
            $($true)*
        } {""})
    };

    (@if_not_flag $target:ident, $($flag:ident)* { $($true:tt)* }) => {
        handler!(@match_flag $target, $($flag)*, {""} {
            $($true)*
        })
    };

    (@if_else_flag $target:ident, $($flag:ident)*, { $($true:tt)* } { $($false:tt)* }) => {
        handler!(@match_flag $target, $($flag)*, {
            $($true)*
        } {
            $($false)*
        })
    };

    (@match_flag $target:ident, $head:ident $($rest:ident)*, { $($true:tt)* } { $($false:tt)* }) => {
        handler!(@flag_check $target $head { $($true)* } {
            handler!(@match_flag $target, $($rest)*, {
                $($true)*
            } {
                $($false)*
            })
        })
    };

    (@match_flag $target:ident,, { $($true:tt)* } { $($false:tt)* }) => {
        $($false)*
    };


    (@flag_check slow_swap slow_swap { $($true:tt)* } { $($false:tt)* }) => {
        $($true)*
    };
    (@flag_check has_code has_code { $($true:tt)* } { $($false:tt)* }) => {
        $($true)*
    };

    (@flag_check $target:ident $head:ident { $($true:tt)* } { $($false:tt)* }) => {
        $($false)*
    };
}

pub extern "C" fn general_interrupt_handler(
    proc_data: &mut InterruptProcessorState,                   //rdi
    atomic_int: u64,                                           //rsi
    main_handler: extern "C" fn(&mut InterruptProcessorState), //rdx
) {
    let mut locals = CpuLocals::get_mut();
    let prev_atomic = locals.atomic_context;
    locals.int_depth += 1;
    locals.atomic_context |= atomic_int != 0;
    let atomic_context = locals.atomic_context;
    let prev_mode = locals.page_fault_handle_mode;
    locals.page_fault_handle_mode = PageFaultHandleMode::KernelPanic;
    drop(locals);

    if !atomic_context {
        enable_interrupts();
    }

    println!(
        "Interrupt received for handler at addr {:x}, atomic: {}",
        main_handler as u64, atomic_context
    );

    main_handler(proc_data);

    //proc is depth 0, root int is depth 1
    let mut locals = CpuLocals::get_mut();
    if locals.int_depth > 1 || locals.atomic_context {
        disable_interrupts();
        locals.int_depth -= 1;
        locals.atomic_context = prev_atomic;
        locals.page_fault_handle_mode = prev_mode;
        return;
    }
    if let Some(curr_proc) = locals.current_process.as_mut() {
        //save current process state
        save_cpu_state(&StackCpuStateData::Interrupt(proc_data), curr_proc);
        release_current_proc(curr_proc, false);
    }
    drop(locals);
    interrupt_context_switch();

    //did not context switch -> PROC not initialized or some other "error"
    disable_interrupts();
    let mut locals = CpuLocals::get_mut();
    locals.int_depth -= 1;
    locals.atomic_context = prev_atomic;
    locals.lock_info.assert_no_locks();
    locals.page_fault_handle_mode = prev_mode;
    drop(locals);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InterruptProcessorState {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub err_code: u64,
    pub interrupt_frame: InterruptFrame,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InterruptFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl InterruptProcessorState {
    pub fn new(rip: u64, rsp: u64) -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            err_code: 0,
            interrupt_frame: InterruptFrame {
                rip,
                cs: 0x23,
                rflags: 0x202,
                rsp,
                ss: 0x1B,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_full(
        r15: u64,
        r14: u64,
        r13: u64,
        r12: u64,
        r11: u64,
        r10: u64,
        r9: u64,
        r8: u64,
        rbp: u64,
        rsp: u64,
        rdi: u64,
        rsi: u64,
        rdx: u64,
        rcx: u64,
        rbx: u64,
        rax: u64,
        rip: u64,
    ) -> Self {
        Self {
            r15,
            r14,
            r13,
            r12,
            r11,
            r10,
            r9,
            r8,
            rbp,
            rdi,
            rsi,
            rdx,
            rcx,
            rbx,
            rax,
            err_code: 0,
            interrupt_frame: InterruptFrame {
                rip,
                cs: 0x23,      //user code segment
                rflags: 0x202, //default flags with interrupts enabled
                rsp,
                ss: 0x1B, //user data segment
            },
        }
    }
}
