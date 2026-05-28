use std::{
    println,
    sync::arc::Arc,
    time::{GET_TIME, UNIX_EPOCH},
};

use crate::{
    memory::safe_memcpy,
    proc::{ProcessData, syscall::SyscallCpuState},
};

pub fn time(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let time = unsafe { GET_TIME() };
    let duration = time.duration_since(UNIX_EPOCH);

    let ptr_seconds = args.get_legacy_syscall_arg(1);
    let ptr_nanos = args.get_legacy_syscall_arg(2);

    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos() as u64;

    let valid_ptrs =
        safe_memcpy(ptr_seconds, (&raw const secs) as u64, 8) && safe_memcpy(ptr_nanos, (&raw const nanos) as u64, 8);
    if !valid_ptrs {
        proc.set_legacy_syscall_return(u64::MAX, 0);
        return false;
    }

    println!("time syscall returning");
    false
}
