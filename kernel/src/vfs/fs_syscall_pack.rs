use core::sync::atomic::Ordering;
use std::boxed::Box;
use std::mem_utils::PhysAddr;
use std::println;
use std::sync::arc::Arc;
use std::vec::Vec;

use crate::memory::{physical_allocator, safe_memcpy_from_user, safe_memcpy_to_user};
use crate::proc::namespaces::FilesystemNamespace;
use crate::proc::syscall::{SyscallCpuState, SyscallPack, register_syscall_pack, string_from_args};
use crate::proc::{self, ProcessData, syscall};
use crate::task_runner::{self, PidOption};
use crate::vfs::file::OpenFlags;
use crate::vfs::{self, InodeIdentifierChain, InodePermissionFlags, InodeType, InodeTypeAndPerms};

pub(super) fn init_fs_syscall_pack() {
    let handlers = [fopen, fclose, fread, fwrite, fseek, fcreate, flink, funlink, fstat];

    let fs_syscalls = SyscallPack::new(Box::new(handlers));
    register_syscall_pack("fs".into(), Arc::new(fs_syscalls));
}

enum FileSeekMode {
    Start = 0,
    Current = 1,
    End = 2,
}

impl FileSeekMode {
    fn from_u64(value: u64) -> Option<Self> {
        match value {
            0 => Some(FileSeekMode::Start),
            1 => Some(FileSeekMode::Current),
            2 => Some(FileSeekMode::End),
            _ => None,
        }
    }
}

fn fopen(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let path_len = args.get_arg(0);
    let path_ptr = args.get_arg(1);
    let fd = args.get_arg(2);
    let flags = OpenFlags(args.get_arg(3));

    let Some(path) = string_from_args(path_ptr, path_len) else {
        println!("fopen: invalid path pointer or length");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(namespace_id)
        .expect("default fs namespace must exist");

    let file_source: InodeIdentifierChain = if fd == 0 {
        fs_namespace.get_cwd_chain()
    } else {
        let Some(f_chain) = fs_namespace.get_whole_chain(fd) else {
            println!("fopen: invalid fd {fd}");
            proc.set_syscall_return(&[u64::MAX]);
            return;
        };
        f_chain
    };

    let pid = proc.pid();

    let proc_clone = proc.downgrade();
    let task = async move {
        let resolved_path = vfs::resolve_path(&path);
        let handle = vfs::open_file((&resolved_path).into(), Some(file_source), flags).await;
        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };
        match handle {
            Ok(handle) => {
                let proc_mut = proc.get_mutable();
                let namespaces = proc_mut.get_namespaces();
                let fs_namespace = namespaces
                    .get_namespace::<FilesystemNamespace>(namespace_id)
                    .expect("default fs namespace must exist");
                let f_descriptor = fs_namespace.open_file_handle(handle);
                drop(proc_mut);
                proc.set_syscall_return(&[f_descriptor]);
            }
            Err(_) => {
                println!("fopen: failed to open file at path {path}");
                proc.set_syscall_return(&[u64::MAX]);
            }
        }
        println!("fopen: finished processing fopen for pid {pid:?}");
        proc::wake_process(pid)
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn fclose(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(namespace_id)
        .expect("default fs namespace must exist");

    if fs_namespace.close_file_handle(fd).is_some() {
        proc.set_syscall_return(&[0]);
    } else {
        proc.set_syscall_return(&[u64::MAX]);
    }
}

fn fread(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let size = args.get_arg(1);
    let buf_ptr = args.get_arg(2);
    let pid = proc.pid();

    if size == 0 {
        proc.set_syscall_return(&[0]);
        return;
    }

    let valid = syscall::verify_memory_range(buf_ptr, buf_ptr + size);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }

    let file_handle = {
        let proc_mut = proc.get_mutable();
        let namespaces = proc_mut.get_namespaces();
        let fs_namespace = namespaces
            .get_namespace::<FilesystemNamespace>(namespace_id)
            .expect("default fs namespace must exist");
        if let Some(f_handle) = fs_namespace.get_file_handle(fd) {
            f_handle
        } else {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = physical_allocator::allocate_contiguius_high(pages);
        let buffers = (0..pages).map(|i| buffer_alloc + (i * 4096)).collect::<Vec<PhysAddr>>();

        let read_result = crate::vfs::read_file(f_handle.get(), &buffers, size).await;
        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };
        let Ok(bytes_read) = read_result else {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        };
        //copy to user buffer
        let dst = buf_ptr;
        let src = std::mem_utils::translate_phys_virt_addr(buffer_alloc).0;
        let valid_copy = safe_memcpy_to_user(dst, src, size as usize);
        if !valid_copy {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }

        //free
        for i in 0..pages {
            unsafe { physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
        }

        //return
        proc.set_syscall_return(&[bytes_read]);
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn fwrite(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let size = args.get_arg(1);
    let buf_ptr = args.get_arg(2);
    let pid = proc.pid();

    if size == 0 {
        proc.set_syscall_return(&[0]);
        return;
    }

    let valid = syscall::verify_memory_range(buf_ptr, buf_ptr + size);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }

    let file_handle = {
        let proc_mut = proc.get_mutable();
        let namespaces = proc_mut.get_namespaces();
        let fs_namespace = namespaces
            .get_namespace::<FilesystemNamespace>(namespace_id)
            .expect("default fs namespace must exist");
        if let Some(f_handle) = fs_namespace.get_file_handle(fd) {
            f_handle
        } else {
            println!("fwrite: invalid fd {fd}");
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = physical_allocator::allocate_contiguius_high(pages);
        let dst = std::mem_utils::translate_phys_virt_addr(buffer_alloc).0 as *mut u8;
        let src = buf_ptr;
        //copy to user buffer
        let copy_valid = safe_memcpy_from_user(dst as u64, src, size as usize);
        if !copy_valid {
            proc.set_syscall_return(&[u64::MAX]);

            //free
            for i in 0..pages {
                unsafe { physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
            }
            return;
        }

        let buffers = (0..pages).map(|i| buffer_alloc + (i * 4096)).collect::<Vec<PhysAddr>>();

        let write_result = crate::vfs::write_file(f_handle.get(), &buffers, size).await;
        let Some(proc) = proc_clone.upgrade() else {
            //free
            for i in 0..pages {
                unsafe { physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
            }
            return; //proc was killed
        };
        if write_result.is_err() {
            println!("fwrite: write failed");
            proc.set_syscall_return(&[u64::MAX]);

            //free
            for i in 0..pages {
                unsafe { physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
            }
            return;
        }
        let bytes_written = unsafe { write_result.unwrap_unchecked() };

        //free
        for i in 0..pages {
            unsafe { physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
        }

        //return
        proc.set_syscall_return(&[bytes_written]);
        println!("fwrite: finished processing fwrite for pid {pid:?}, bytes_written: {bytes_written}");
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn fseek(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let offset = args.get_arg(1) as i64;
    let mode_value = args.get_arg(2);
    let pid = proc.pid();

    let Some(mode) = FileSeekMode::from_u64(mode_value) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let file_handle = {
        let proc_mut = proc.get_mutable();
        let namespaces = proc_mut.get_namespaces();
        let fs_namespace = namespaces
            .get_namespace::<FilesystemNamespace>(namespace_id)
            .expect("default fs namespace must exist");
        if let Some(f_handle) = fs_namespace.get_file_handle(fd) {
            f_handle
        } else {
            println!("fseek: invalid fd {fd}");
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let inode = file_handle.open_file.inode.lock().await;
        let f_size = inode.size;
        let current_pos = file_handle.position.load(Ordering::Acquire);

        let new_pos = match mode {
            FileSeekMode::Start => offset,
            FileSeekMode::Current => current_pos as i64 + offset,
            FileSeekMode::End => f_size as i64 + offset,
        };

        let new_pos = new_pos.max(0).min(f_size as i64) as u64;
        file_handle.position.store(new_pos, Ordering::Release);

        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };
        proc.set_syscall_return(&[new_pos]);
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn fcreate(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let name_len = args.get_arg(0);
    let name_ptr = args.get_arg(1);
    let fd = args.get_arg(2);
    let Some(inode_type) = InodeType::from_id(args.get_arg(3) as u32) else {
        println!("fcreate: invalid inode type id");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };
    let perms = InodePermissionFlags((args.get_arg(4) & 0xFF_FF_FF) as u32);
    let type_perms = InodeTypeAndPerms::new(inode_type, perms);

    let Some(name) = string_from_args(name_ptr, name_len) else {
        println!("fcreate: invalid name pointer or length");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let pid = proc.pid();

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(namespace_id)
        .expect("default fs namespace must exist");

    let Some(parent_dir) = fs_namespace.get_file_handle(fd) else {
        println!("fcreate: invalid fd {fd}");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let res = vfs::create_file(&parent_dir, &name, type_perms).await;

        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };
        if let Err(e) = res {
            println!("fcreate: failed to create file: {e}");
            proc.set_syscall_return(&[u64::MAX]);
            crate::proc::wake_process(proc.pid());
            return;
        }
        proc.set_syscall_return(&[0]);
        crate::proc::wake_process(proc.pid());
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn flink(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let name_len = args.get_arg(0);
    let name_ptr = args.get_arg(1);
    let parent_fd = args.get_arg(2);
    let target_fd = args.get_arg(3);

    let Some(name) = string_from_args(name_ptr, name_len) else {
        println!("fcreate: invalid name pointer or length");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let pid = proc.pid();

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(namespace_id)
        .expect("default fs namespace must exist");

    let Some(parent_dir) = fs_namespace.get_file_handle(parent_fd) else {
        println!("fcreate: invalid fd {parent_fd}");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };
    let Some(target_file) = fs_namespace.get_file_handle(target_fd) else {
        println!("fcreate: invalid fd {target_fd}");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let res = vfs::link_file(&parent_dir, &name, &target_file).await;

        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };

        if let Err(e) = res {
            println!("fcreate: failed to create file: {e}");
            proc.set_syscall_return(&[u64::MAX]);
            crate::proc::wake_process(proc.pid());
            return;
        }
        proc.set_syscall_return(&[0]);
        crate::proc::wake_process(proc.pid());
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn funlink(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let name_len = args.get_arg(0);
    let name_ptr = args.get_arg(1);
    let parent_fd = args.get_arg(2);

    let Some(name) = string_from_args(name_ptr, name_len) else {
        println!("funlink: invalid name pointer or length");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let pid = proc.pid();

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(namespace_id)
        .expect("default fs namespace must exist");

    let Some(parent_dir) = fs_namespace.get_file_handle(parent_fd) else {
        println!("funlink: invalid fd {parent_fd}");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let res = vfs::unlink_file(&parent_dir, &name).await;

        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };
        if let Err(e) = res {
            println!("funlink: failed to unlink file: {e}");
            proc.set_syscall_return(&[u64::MAX]);
            crate::proc::wake_process(proc.pid());
            return;
        }
        proc.set_syscall_return(&[0]);
        crate::proc::wake_process(proc.pid());
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}

fn fstat(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let buf_ptr = args.get_arg(1);

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let fs_namespace = namespaces
        .get_namespace::<FilesystemNamespace>(namespace_id)
        .expect("default fs namespace must exist");
    let Some(file_handle) = fs_namespace.get_file_handle(fd) else {
        println!("fstat: invalid fd {fd}");
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let pid = proc.pid();

    let proc_clone = proc.downgrade();
    let task = async move {
        let stat_result = vfs::stat_file(file_handle.get()).await;

        let Some(proc) = proc_clone.upgrade() else {
            return; //proc was killed
        };
        let dst = buf_ptr;
        let src = (&raw const stat_result) as u64;
        let valid_copy = safe_memcpy_to_user(dst, src, core::mem::size_of::<vfs::Inode>() as usize);
        if !valid_copy {
            println!("fstat: failed to copy stat result to user buffer");
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }

        proc.set_syscall_return(&[0]);
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}
