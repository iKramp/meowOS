use super::{context_switch::no_ret_context_switch, process_data::StackCpuStateData, scheduler::{save_and_release_current, SleepCondition}};
use crate::{interrupts::enable_interrupts, msr, proc::syscall};

mod handlers;

const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_CSTAR: u32 = 0xC000_0083;
const MSR_SFMASK: u32 = 0xC000_0084;
const MSR_EFER: u32 = 0xC000_0080;

///Prepare all necessary things for executing syscalls. This includes setting interrupt handlers,
///MSRs and more
pub(super) fn init() {
    let syscall_cs_ss: u16 = 0x8;
    let sysret_cs_ss: u16 = 0x10 | 0x3;
    let syscall_eip: u64 = 0; //unused
    let syscall_rip: u64 = handler_wrapper as *const fn() as u64;
    let compat_rip: u64 = 0; //unused
    let syscall_flag_mask: u32 = 0x700;

    let mut star_reg = (sysret_cs_ss as u64) << 48;
    star_reg |= (syscall_cs_ss as u64) << 32;
    star_reg |= syscall_eip;

    msr::set_msr(MSR_STAR, star_reg);
    msr::set_msr(MSR_LSTAR, syscall_rip);
    msr::set_msr(MSR_CSTAR, compat_rip);
    msr::set_msr(MSR_SFMASK, syscall_flag_mask as u64);
    enable_syscall();
}

fn enable_syscall() {
    let mut efer = msr::get_msr(MSR_EFER);
    efer |= 1;
    msr::set_msr(MSR_EFER, efer);
}

//sys V abi:
//ret val: rax, rdx
//parameters: rdi, rsi, rdx, rcx, r8, r9
//scratch regs: rax, rdi, rsi, rdx, rcx, r8, r9, r10, r11
//preserved: rbx, rsp, rbp, r12 - r15

//linux syscall abi:
//ret val: rax, rdx
//parameters: rdi, rsi, rdx, r10, r8, r9
//syscall number: rax
//x86-reserved: rcx, r11
//preserved: rbx, rbp, r12 - r15

//syscalls are limited to 5 64bit parameters. If more data is needed, set up a structure and pass a
//pointer to it
#[naked]
extern "C" fn handler_wrapper() -> ! {
    //INFO: any kind of change here should be matched with the one in dispatcher.rs
    unsafe { core::arch::naked_asm!(
        //push preserved regs, get kernel stack from gsbase
        //stack is aligned to 16 here
        "swapgs",

        "mov gs:[16], rcx", //save user rip to gsbase area
        "mov cx, 0",
        "mov ss, cx",
        "mov rcx, gs:[16]", //get user rip from gsbase area

        "mov gs:[16], rsp", //save user rsp to gsbase area
        "mov rsp, gs:[8]", //get kernel rsp from gsbase area

        "sub rsp, 8*8",
        "mov [rsp + 8*7], rbx",
        "mov [rsp + 8*6], rbp",
        "mov [rsp + 8*5], r12",
        "mov [rsp + 8*4], r13",
        "mov [rsp + 8*3], r14",
        "mov [rsp + 8*2], r15",
        "mov [rsp + 8*1], r11", //rflags is in r11
        "mov [rsp + 8*0], rcx", //return rip

        //push args too
        "sub rsp, 8*7",
        "mov [rsp + 8*6], rax", //syscall number
        "mov [rsp + 8*5], r9",
        "mov [rsp + 8*4], r8",
        "mov [rsp + 8*3], r10",
        "mov [rsp + 8*2], rdx",
        "mov [rsp + 8*1], rsi",
        "mov [rsp + 8*0], rdi",

        "mov rdi, rsp", //args rsp



        "call {}",
        sym handler
    )}
}

#[allow(unused_variables)]
extern "C" fn handler(args_rsp: u64) -> ! {
    //handle here
    // println!("Syscall called with args: {}, {}, {}, {}", arg1, arg2, arg3, arg4);

    let args_ptr = args_rsp as *mut u64;
    let state_ptr = unsafe { args_ptr.byte_add(core::mem::size_of::<SyscallArgs>()).sub(2) }; //2 regs overlap

    let args = unsafe { &mut *(args_ptr as *mut SyscallArgs) };
    let state = unsafe { &*(state_ptr as *const SyscallCpuState) };

    let locals = crate::acpi::cpu_locals::CpuLocals::get();
    unsafe { core::ptr::addr_of_mut!(locals.int_depth).write_volatile(1) };
    let curr_proc = locals.current_process.as_mut().expect("syscalled while no current process in locals");
    enable_interrupts();

    #[allow(clippy::single_match)]
    let task_sleep = match args.syscall_number {
        0 => syscall::handlers::illegal(args, curr_proc),
        1 => todo!("implement exit"),
        2 => todo!("implement exec"),
        3 => todo!("implement clone"),
        4 => syscall::handlers::fopen(args, curr_proc),
        5 => syscall::handlers::fclose(args, curr_proc),
        6 => syscall::handlers::fread(args, curr_proc),
        7 => syscall::handlers::fwrite(args, curr_proc),
        8 => todo!("implement fseek"),
        9 => todo!("implement mmap"),
        10 => todo!("implement munmap"),
        11 => todo!("implement sleep"),
        12 => syscall::handlers::time(args),
        _ => {false}
    };

    let sleep_cond = if task_sleep {
        Some(SleepCondition::Event)
    } else {
        None
    };

    save_and_release_current(curr_proc, &StackCpuStateData::Syscall(state), sleep_cond);
    no_ret_context_switch();
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SyscallCpuState {
    pub rdx: u64,
    pub rax: u64,
    pub rcx: u64,
    pub r11: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64
}

#[repr(C)]
struct SyscallArgs {
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
    syscall_number: u64,
}
