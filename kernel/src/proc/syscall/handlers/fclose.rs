use std::sync::arc::Arc;

use crate::proc::{FilesystemNamespace, ProcessData, syscall::SyscallCpuState};

pub fn fclose(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let fd = args.get_legacy_syscall_arg(1);
    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(0)
        .expect("default fs namespace must exist");

    if fs_namespace.close_file_handle(fd).is_some() {
        proc.set_legacy_syscall_return(0, 0);
    } else {
        proc.set_legacy_syscall_return(u64::MAX, 1);
    }
    false
}
