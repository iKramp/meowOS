use std::sync::arc::Arc;

use crate::proc::{self, ProcessData, syscall::NewSyscallCpuState};

pub fn exit(args: &mut NewSyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let status = args.get_legacy_syscall_arg(1);
    proc::kill_process(proc.pid(), status);
    false
}
