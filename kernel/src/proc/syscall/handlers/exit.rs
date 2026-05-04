use std::sync::arc::Arc;

use crate::proc::{self, ProcessData, syscall::SyscallCpuState};

pub fn exit(args: &mut SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let status = args.get_legacy_syscall_arg(1);
    proc::kill_process(proc.pid(), status);
    false
}
