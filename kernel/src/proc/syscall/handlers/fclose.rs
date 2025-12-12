use std::sync::arc::Arc;

use crate::proc::{syscall::SyscallArgs, ProcessData};


pub fn fclose(args: &mut SyscallArgs, proc: &Arc<ProcessData>) -> bool {
    let fd = args.arg1;
    let mut proc_mut = proc.get_mutable();
    if proc_mut.take_file_handle(fd).is_some() {
        args.arg1 = 0;
        args.arg2 = 0;
    } else {
        args.arg1 = u64::MAX;
        args.arg2 = 1;
    }
    false
}
