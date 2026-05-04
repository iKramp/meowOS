use std::println;

use super::{
    context_switch::no_ret_context_switch,
    process_data::StackCpuStateData,
    scheduler::{SleepCondition, release_current_proc, save_cpu_state},
};
use crate::{
    acpi::cpu_locals::PageFaultHandleMode,
    interrupts::enable_interrupts,
    memory, msr,
    proc::syscall::{self, legacy_syscall_pack::init_legacy_syscalls},
};

mod handlers;
mod legacy_syscall_pack;
mod syscall_registry;
pub use syscall_registry::*;

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

    init_legacy_syscalls();

    enable_syscall();
}

fn enable_syscall() {
    let mut efer = msr::get_msr(MSR_EFER);
    efer |= 1;
    msr::set_msr(MSR_EFER, efer);
}

///performs an exclusive range check if the pointers are valid in userspace
fn verify_memory_range(mem_start: u64, mem_end: u64) -> bool {
    if mem_start > mem_end || mem_end > 0x0000_8000_0000_0000 {
        println!(level:warn, "Invalid memory range: {:#X} - {:#X}", mem_start, mem_end);
        return false;
    }

    let valid = memory::probe_pointer_range(mem_start, mem_end);
    if !valid {
        println!(level:warn, "Invalid memory range: {:#X} - {:#X}", mem_start, mem_end);
    }
    valid
}

fn verify_memory_ptr(mut ptr: u64) -> bool {
    ptr &= !0xFFF; //page align, ptr can't overlap pages because of alignment
    if ptr > 0x0000_8000_0000_0000 {
        println!(level:warn, "Invalid memory pointer: {:#X}", ptr);
        return false;
    }
    let valid = memory::probe_ptr_u64(ptr).is_some();
    if !valid {
        println!(level:warn, "Invalid memory pointer: {:#X}", ptr);
    }
    valid
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
    unsafe {
        core::arch::naked_asm!(
            //push preserved regs, get kernel stack from gsbase
            //stack is aligned to 16 here
            "swapgs",

            "mov gs:[16], rcx", //save user rip to gsbase area
            "mov cx, 0",
            "mov ss, cx",
            "mov rcx, gs:[16]", //get user rip from gsbase area

            "mov gs:[16], rsp", //save user rsp to gsbase area
            "mov rsp, gs:[8]", //get kernel rsp from gsbase area

            // "sub rsp, 8*8",
            // "mov [rsp + 8*7], rbx",
            // "mov [rsp + 8*6], rbp",
            // "mov [rsp + 8*5], r12",
            // "mov [rsp + 8*4], r13",
            // "mov [rsp + 8*3], r14",
            // "mov [rsp + 8*2], r15",
            // "mov [rsp + 8*1], r11", //rflags is in r11
            // "mov [rsp + 8*0], rcx", //return rip
            //
            // //push args too
            // "sub rsp, 8*7",
            // "mov [rsp + 8*6], rax", //syscall number
            // "mov [rsp + 8*5], r9",
            // "mov [rsp + 8*4], r8",
            // "mov [rsp + 8*3], r10",
            // "mov [rsp + 8*2], rdx",
            // "mov [rsp + 8*1], rsi",
            // "mov [rsp + 8*0], rdi",
            //
            // "mov rdi, rsp", //args rsp

            "sub rsp, 8*16", //space for 16 u64s
            "mov [rsp + 0*8], rdi",
            "mov [rsp + 1*8], rsi",
            "mov [rsp + 2*8], rbp",
            "mov rdi, gs:[16]", //user rsp
            "mov [rsp + 3*8], rdi", //user rsp
            "mov [rsp + 4*8], rax",
            "mov [rsp + 5*8], rbx",
            "mov [rsp + 6*8], rcx",
            "mov [rsp + 7*8], rdx",
            "mov [rsp + 8*8], r8",
            "mov [rsp + 9*8], r9",
            "mov [rsp + 10*8], r10",
            "mov [rsp + 11*8], r11",
            "mov [rsp + 12*8], r12",
            "mov [rsp + 13*8], r13",
            "mov [rsp + 14*8], r14",
            "mov [rsp + 15*8], r15",

            "mov rdi, rsp", //args rsp

            "call {}",
            sym handler
        )
    }
}

#[allow(unused_variables)]
extern "C" fn handler(saved_regs_ptr: u64) -> ! {
    //handle here
    // println!("Syscall called with args: {}, {}, {}, {}", arg1, arg2, arg3, arg4);

    let saved_regs_ptr = saved_regs_ptr as *mut u64;

    let saved_regs = unsafe { &mut *(saved_regs_ptr as *mut SyscallCpuState) };

    let mut locals = crate::acpi::cpu_locals::CpuLocals::get_mut();
    unsafe { core::ptr::addr_of_mut!(locals.int_depth).write_volatile(1) };
    locals.page_fault_handle_mode = PageFaultHandleMode::KernelPanic;
    let curr_proc = locals
        .current_process
        .as_mut()
        .expect("syscalled while no current process in locals")
        .clone();
    drop(locals);
    enable_interrupts();

    save_cpu_state(&StackCpuStateData::Syscall(saved_regs), &curr_proc);

    let syscall_number = saved_regs.rax;
    println!("Syscall number: {}, state: {:#X?}", syscall_number, saved_regs);

    let syscall_namespace = curr_proc.get_mutable().get_namespaces().get_syscall_namespace();
    let syscall_handler = syscall_namespace.get_syscall_handler(syscall_number as u32);
    let task_sleep;
    if let Some(syscall_handler) = syscall_handler {
        task_sleep = syscall_handler(saved_regs, &curr_proc);
    } else {
        println!(level:warn, "Invalid syscall number: {}", syscall_number);
        syscall::handlers::illegal(saved_regs, &curr_proc);
        task_sleep = false;
    };

    let sleep_cond = if task_sleep { Some(SleepCondition::Event) } else { None };

    release_current_proc(&curr_proc, sleep_cond);
    no_ret_context_switch();
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SyscallCpuState {
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl SyscallCpuState {
    pub fn get_legacy_syscall_arg(&self, index: usize) -> u64 {
        match index {
            1 => self.rdi,
            2 => self.rsi,
            3 => self.rdx,
            4 => self.r10,
            5 => self.r8,
            6 => self.r9,
            _ => panic!("Invalid legacy syscall argument index: {}", index),
        }
    }
}
