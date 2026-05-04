use std::{
    println,
    sync::arc::Arc,
    time::{GET_TIME, UNIX_EPOCH},
};

use crate::proc::{
    ProcessData,
    syscall::{self, NewSyscallCpuState},
};

pub fn time(args: &mut NewSyscallCpuState, _proc: &Arc<ProcessData>) -> bool {
    let time = unsafe { GET_TIME() };
    let duration = time.duration_since(UNIX_EPOCH);

    let ptr_seconds = args.get_legacy_syscall_arg(1);
    let ptr_nanos = args.get_legacy_syscall_arg(2);

    let valid_ptrs = syscall::verify_memory_ptr(ptr_seconds) && syscall::verify_memory_ptr(ptr_nanos);
    if !valid_ptrs {
        args.set_legacy_syscall_ret(u64::MAX, 0);
        return false;
    }

    unsafe {
        *(ptr_seconds as *mut u64) = duration.as_secs();
        *(ptr_nanos as *mut u64) = duration.subsec_nanos() as u64;
    }

    println!("time syscall returning");
    false
}
