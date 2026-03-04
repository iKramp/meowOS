use core::{slice, str};
use std::{boxed::Box, sync::arc::Arc, vec::Vec};

use crate::{
    proc::{
        self, ProcessData,
        syscall::{self, SyscallArgs},
    },
    task_runner,
    vfs::{self, InodeIdentifierChain, file::FileFlags},
};

pub fn fopen(args: &mut SyscallArgs, proc: &Arc<ProcessData>) -> bool {
    let pid = proc.pid();
    let path_len = args.arg1;
    let path_ptr = args.arg2;
    let fd = args.arg3;
    let ftags = args.arg4;
    let _create_mode = args.arg5;

    let res = syscall::verify_memory(path_ptr, path_ptr + path_len);
    if !res {
        args.syscall_number = u64::MAX;
        return false;
    }

    let Ok(path) = (unsafe { str::from_utf8(slice::from_raw_parts(path_ptr as *const u8, path_len as usize)) }) else {
        args.syscall_number = u64::MAX;
        return false;
    };

    let file_source: Option<InodeIdentifierChain> = if fd == 0 {
        None
    } else {
        let proc_mut = proc.get_mutable();
        let Some(f_handle) = proc_mut.get_file_handle(fd) else {
            args.syscall_number = u64::MAX;
            return false;
        };
        let mut new_chain = Vec::from(f_handle.parent_chain.as_ref());
        new_chain.push(f_handle.inode);
        Some(new_chain.into_boxed_slice())
    };

    let task = async move {
        let resolved_path = vfs::resolve_path(path);
        let file_flags = FileFlags(ftags as u8);
        let handle = vfs::open_file((&resolved_path).into(), file_source, file_flags).await;
        let Some(proc) = crate::proc::get_proc(pid) else {
            return; //proc was killed
        };
        match handle {
            Ok(handle) => {
                let proc_lock = proc.get();
                let f_descriptor = proc_lock.open_file_handle(handle);
                proc_lock.set_syscall_return(f_descriptor, 0);
            }
            Err(_) => {
                let proc_lock = proc.get();
                proc_lock.set_syscall_return(u64::MAX, 1);
            }
        }
        proc::wake_process(pid)
    };

    task_runner::add_task(Box::pin(task), Some(pid));
    true
}
