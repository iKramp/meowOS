use std::sync::arc::Arc;

use crate::proc::{ProcessData, syscall::SyscallCpuState};

pub fn fclose(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let fd = args.get_legacy_syscall_arg(1);
    let mut proc_mut = proc.get_mutable();
    if proc_mut.take_file_handle(fd).is_some() {
        proc.set_legacy_syscall_return(0, 0);
    } else {
        proc.set_legacy_syscall_return(u64::MAX, 1);
    }
    false
}
