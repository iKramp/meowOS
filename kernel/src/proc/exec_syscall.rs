use std::{boxed::Box, sync::arc::Arc};

use crate::proc::{
    NamespaceIds, ProcessData,
    context::{builder::create_process_from_parts, parts::X86RegisterState},
    syscall::{SyscallCpuState, SyscallHandler, SyscallPack},
};

pub fn init_exec_syscall() {
    let handlers: [SyscallHandler; _] = [exec];

    let exec_syscall_pack = SyscallPack::new(Box::new(handlers));
    crate::proc::syscall::register_syscall_pack("exec".into(), Arc::new(exec_syscall_pack));
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
