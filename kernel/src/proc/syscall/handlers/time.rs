use std::{println, sync::arc::Arc, time::{GET_TIME, UNIX_EPOCH}};

use crate::proc::{ProcessData, syscall::{self, SyscallArgs}};


pub fn time(args: &mut SyscallArgs, _proc: &Arc<ProcessData>) -> bool {
    let time = unsafe { GET_TIME() };
    let duration = time.duration_since(UNIX_EPOCH);

    let ptr_seconds = args.arg1;
    let ptr_nanos = args.arg2;

    let valid_ptrs = syscall::verify_memory_ptr(ptr_seconds) && syscall::verify_memory_ptr(ptr_nanos);
    if !valid_ptrs {
        args.arg1 = u64::MAX;
        return false;
    }

    unsafe {
        *(args.arg1 as *mut u64) = duration.as_secs();
        *(args.arg2 as *mut u64) = duration.subsec_nanos() as u64;
    }

    println!("time syscall returning");
    false
}
