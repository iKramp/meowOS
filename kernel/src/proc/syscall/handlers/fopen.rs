use std::{println, sync::arc::Arc};

use crate::{
    proc::{
        self, FilesystemNamespace, ProcessData,
        syscall::{SyscallCpuState, string_from_args},
    },
    task_runner::{self, PidOption},
    vfs::{
        self, InodeIdentifierChain,
        file::{FileFlags, OpenFlags},
    },
};

pub fn fopen(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let pid = proc.pid();
    let path_len = args.get_legacy_syscall_arg(1);
    let path_ptr = args.get_legacy_syscall_arg(2);
    let fd = args.get_legacy_syscall_arg(3);
    let flags = args.get_legacy_syscall_arg(4);
    let _create_mode = args.get_legacy_syscall_arg(5);

    let Some(path) = string_from_args(path_ptr, path_len) else {
        println!("fopen: invalid path pointer or length");
        proc.set_legacy_syscall_return(u64::MAX, 1);
        return;
    };

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(0)
        .expect("default fs namespace must exist");

    let file_source: InodeIdentifierChain = if fd == 0 {
        fs_namespace.get_cwd_chain()
    } else {
        let Some(f_chain) = fs_namespace.get_whole_chain(fd) else {
            println!("fopen: invalid fd {fd}");
            proc.set_legacy_syscall_return(u64::MAX, 1);
            return;
        };
        f_chain
    };

    let task = async move {
        let resolved_path = vfs::resolve_path(&path);
        let file_flags = FileFlags(flags as u8);

        //legacy BS because open flags are different
        let mut open_flags = OpenFlags(0);
        open_flags
            .set_read(file_flags.read())
            .set_write(file_flags.write())
            .set_append(file_flags.append());

        let handle = vfs::open_file((&resolved_path).into(), Some(file_source), open_flags).await;
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
    proc.set_sleeping(true);
}
