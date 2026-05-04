use std::sync::arc::Arc;

use crate::proc::{ProcessData, syscall::SyscallCpuState};

pub fn fclose(args: &mut SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let fd = args.get_legacy_syscall_arg(1);
    let mut proc_mut = proc.get_mutable();
    if proc_mut.take_file_handle(fd).is_some() {
        args.set_legacy_syscall_ret(0, 0);
    } else {
        args.set_legacy_syscall_ret(u64::MAX, 1);
    }
    false
}
