use std::sync::arc::Arc;

use crate::proc::{ProcessData, syscall::SyscallArgs};

//purely to catch bugs from processes, will always set error
pub fn illegal(_args: &mut SyscallArgs, proc: &Arc<ProcessData>) -> bool {
    proc.set_syscall_return(u64::MAX, 1);
    false
}
