use std::{boxed::Box, error::ErrorCode, lock_w_info, string::ToString, sync::arc::Arc};

use crate::{
    interrupts::InterruptProcessorState,
    proc::{
        NamespaceIds, PROCESS_ID_COUNTER, Pid, ProcNamespaces, ProcessData, SCHEDULER,
        process_data::CpuStateType,
        syscall::{SyscallCpuState, SyscallHandler, SyscallPack},
    },
};

pub fn init_exec_syscall() {
    let handlers: [SyscallHandler; _] = [exec];

    let exec_syscall_pack = SyscallPack::new(Box::new(handlers));
    crate::proc::syscall::register_syscall_pack("exec".into(), Arc::new(exec_syscall_pack));
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct X86RegisterState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

#[repr(C)]
union RegisterState {
    x86: X86RegisterState,
    //other architectures here
}

#[repr(C)]
struct ExecArgs {
    namespaces: NamespaceIds,
    registers: RegisterState,
    start_ptr: u64,
    name_len: u64,
    name_ptr: u64,
}

fn exec(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let exec_args_ptr = args.get_arg(0);
    let valid = crate::proc::syscall::verify_memory_range(exec_args_ptr, exec_args_ptr + core::mem::size_of::<ExecArgs>() as u64);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    let exec_args = unsafe { &*(exec_args_ptr as *const ExecArgs) };
    let valid = crate::proc::syscall::verify_memory_range(exec_args.name_ptr, exec_args.name_ptr + exec_args.name_len);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    let name = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            exec_args.name_ptr as *const u8,
            exec_args.name_len as usize,
        ))
    };

    let Ok(namespaces) = proc
        .get_mutable()
        .get_namespaces()
        .clone_from_ids(exec_args.namespaces.clone())
    else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let Ok(pid) = create_process_from_parts(unsafe { exec_args.registers.x86 }, exec_args.start_ptr, namespaces, name) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    proc.set_syscall_return(&[pid.0 as u64]);
    false
}

pub fn create_process_from_parts(
    register_states: X86RegisterState,
    start_ptr: u64,
    namespaces: ProcNamespaces,
    name: &str,
) -> Result<Pid, ErrorCode> {
    let cpu_state = InterruptProcessorState::new_full(
        register_states.r15,
        register_states.r14,
        register_states.r13,
        register_states.r12,
        register_states.r11,
        register_states.r10,
        register_states.r9,
        register_states.r8,
        register_states.rbp,
        register_states.rsp,
        register_states.rdi,
        register_states.rsi,
        register_states.rdx,
        register_states.rcx,
        register_states.rbx,
        register_states.rax,
        start_ptr,
    );

    let pid = Pid(PROCESS_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed));
    let process_data = ProcessData::new(
        pid,
        false,
        name.to_string().into_boxed_str(),
        CpuStateType::Interrupt(cpu_state),
        namespaces,
    );

    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.accept_new_process(pid, process_data);

    Ok(pid)
}
