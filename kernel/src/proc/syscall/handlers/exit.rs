use std::sync::arc::Arc;

use crate::proc::{self, ProcessData, syscall::SyscallArgs};


pub fn illegal(args: &mut SyscallArgs, proc: &Arc<ProcessData>) -> bool {
    let status = args.arg1;
    proc::kill_process(proc.pid(), status);
    false
}
