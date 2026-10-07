use crate::arch::memory::PAGE_OFFSET_MASK;
use crate::arch::memory::VIRTUAL_ADDRESS_BITS;
use crate::arch::syscall as arch_syscall;
use crate::arch::syscall::cpu_syscall_init;
use crate::clocks::cancel_scheduled_event;
pub use arch_syscall::SyscallCpuState;
pub use arch_syscall::return_syscalled;
use std::{boxed::Box, println, string::String};

use super::{
    context_switch::no_ret_context_switch,
    process_data::StackCpuStateData,
    scheduler::{release_current_proc, save_cpu_state},
};
use crate::{
    acpi::cpu_locals::CpuLocals,
    interrupts::enable_interrupts,
    memory,
    proc::{self, SyscallNamespace},
};

mod syscall_management_pack;
mod syscall_registry;
pub use syscall_registry::*;

const KERNEL_START_ADDR: u64 = 1 << (VIRTUAL_ADDRESS_BITS - 1);

///Prepare all necessary things for executing syscalls. This includes setting interrupt handlers,
///MSRs and more
pub(super) fn init() {
    syscall_management_pack::init_syscall_management_syscalls();

    cpu_syscall_init();
}

///performs an exclusive range check if the pointers are valid in userspace
pub fn verify_memory_range(mem_start: u64, mem_end: u64) -> bool {
    if mem_start > mem_end || mem_end > KERNEL_START_ADDR {
        println!(level:warn, "Invalid memory range: {:#X} - {:#X}", mem_start, mem_end);
        return false;
    }

    let valid = memory::probe_pointer_range(mem_start, mem_end);
    if !valid {
        println!(level:warn, "Invalid memory range: {:#X} - {:#X}", mem_start, mem_end);
    }
    valid
}

pub fn verify_memory_ptr(mut ptr: u64) -> bool {
    ptr &= !PAGE_OFFSET_MASK; //page align, ptr can't overlap pages because of alignment
    if ptr >= KERNEL_START_ADDR {
        println!(level:warn, "Invalid memory pointer: {:#X}", ptr);
        return false;
    }
    let valid = memory::probe_ptr_u64(ptr).is_some();
    if !valid {
        println!(level:warn, "Invalid memory pointer: {:#X}", ptr);
    }
    valid
}

pub fn string_from_args(ptr: u64, len: u64) -> Option<String> {
    let str_buf_uninit = Box::new_uninit_slice(len as usize);
    let res = memory::safe_memcpy_from_user(str_buf_uninit.as_ptr() as u64, ptr, len as usize);
    if !res {
        println!(level:warn, "Invalid string pointer or length: {:#X}, {}", ptr, len);
        return None;
    }
    let str_buf: Box<[u8]> = unsafe { str_buf_uninit.assume_init() };
    let Ok(string) = String::from_utf8(str_buf.to_vec()) else {
        println!(level:warn, "Invalid string data (not utf8) at pointer: {:#X}, length: {}", ptr, len);
        return None;
    };
    Some(string)
}

#[allow(unused_variables)]
pub extern "C" fn main_syscall_handler(saved_regs_ptr: u64) -> ! {
    let saved_regs_ptr = saved_regs_ptr as *mut u64;

    let saved_regs = unsafe { &mut *(saved_regs_ptr as *mut SyscallCpuState) };

    let mut locals = CpuLocals::get_mut();
    locals.int_depth = 1;
    let curr_proc = locals
        .current_process
        .as_mut()
        .expect("syscalled while no current process in locals")
        .clone();

    let preemption_id = locals.preemtion_id.take();
    drop(locals);

    if let Some(preemption_id) = preemption_id {
        cancel_scheduled_event(preemption_id);
    }

    enable_interrupts();

    save_cpu_state(&StackCpuStateData::Syscall(saved_regs), &curr_proc);

    let syscall_number = saved_regs.get_syscall_number();
    println!("Syscall number: {:X}, state: {:#X?}", syscall_number, saved_regs);

    let Some(syscall_namespace) = curr_proc.get_mutable().get_namespaces().get_namespace::<SyscallNamespace>(0) else {
        proc::kill_process(curr_proc.pid(), u64::MAX);
        release_current_proc(&curr_proc);
        no_ret_context_switch();
    };
    let syscall_handler = syscall_namespace.get_syscall_handler(syscall_number as u32);
    drop(syscall_namespace);

    if let Some(syscall_handler) = syscall_handler {
        syscall_handler(saved_regs, &curr_proc);
    } else {
        println!(level:warn, "Invalid syscall number: {}", syscall_number);
        curr_proc.set_syscall_return(&[u64::MAX, 0]);
    };

    release_current_proc(&curr_proc);
    no_ret_context_switch();
}
