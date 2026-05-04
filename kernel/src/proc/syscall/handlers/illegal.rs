use std::sync::arc::Arc;

use crate::proc::{ProcessData, syscall::SyscallCpuState};

//purely to catch bugs from processes, will always set error
pub fn illegal(_args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    proc.set_syscall_return(u64::MAX, 1);
    false
}
