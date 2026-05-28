use core::str;
use std::{boxed::Box, println, string::String, sync::arc::Arc};

use crate::{
    memory::safe_memcpy,
    proc::{self, FilesystemNamespace, ProcessData, syscall::SyscallCpuState},
    task_runner::{self, PidOption},
    vfs::{self, InodeIdentifierChain, file::FileFlags},
};

pub fn fopen(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let pid = proc.pid();
    let path_len = args.get_legacy_syscall_arg(1);
    let path_ptr = args.get_legacy_syscall_arg(2);
    let fd = args.get_legacy_syscall_arg(3);
    let ftags = args.get_legacy_syscall_arg(4);
    let _create_mode = args.get_legacy_syscall_arg(5);

    // let path_buf = Box::new([0u8; path_len as usize]);
    let path_buf_uninit = Box::new_uninit_slice(path_len as usize);
    let res = safe_memcpy(path_buf_uninit.as_ptr() as u64, path_ptr, path_len as usize);
    if !res {
        println!("fopen: invalid path pointer or length");
        proc.set_legacy_syscall_return(u64::MAX, 1);
        return false;
    }
    let path_buf: Box<[u8]> = unsafe { path_buf_uninit.assume_init() };

    let Ok(path) = String::from_utf8(path_buf.to_vec()) else {
        println!("fopen: invalid path string (not utf8)");
        proc.set_legacy_syscall_return(u64::MAX, 1);
        return false;
    };

    let file_source: Option<InodeIdentifierChain> = if fd == 0 {
        None
    } else {
        let proc_mut = proc.get_mutable();
        let namespaces = proc_mut.get_namespaces();
        let fs_namespace = namespaces
            .get_namespace::<FilesystemNamespace>(0)
            .expect("default fs namespace must exist");
        let Some(f_chain) = fs_namespace.get_whole_chain(fd) else {
            println!("fopen: invalid fd {fd}");
            proc.set_legacy_syscall_return(u64::MAX, 1);
            return false;
        };
        Some(f_chain)
    };

    let task = async move {
        let resolved_path = vfs::resolve_path(&path);
        let file_flags = FileFlags(ftags as u8);
        let handle = vfs::open_file((&resolved_path).into(), file_source, file_flags).await;
        let Some(proc) = crate::proc::get_proc(pid) else {
            return; //proc was killed
        };
        match handle {
            Ok(handle) => {
                let proc_mut = proc.get_mutable();
                let namespaces = proc_mut.get_namespaces();
                let fs_namespace = namespaces
                    .get_namespace::<FilesystemNamespace>(0)
                    .expect("default fs namespace must exist");
                let f_descriptor = fs_namespace.open_file_handle(handle);
                drop(proc_mut);
                proc.get().set_legacy_syscall_return(f_descriptor, 0);
            }
            Err(_) => {
                println!("fopen: failed to open file at path {path}");
                let proc_lock = proc.get();
                proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            }
        }
        println!("fopen: finished processing fopen for pid {pid:?}");
        proc::wake_process(pid)
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    true
}
