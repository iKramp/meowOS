use std::{sync::arc::Arc, vec::Vec};

use crate::{
    memory::{addresses::*, safe_memcpy_to_user},
    proc::{
        ProcessData,
        syscall::{self, SyscallCpuState},
    },
    task_runner::{self, PidOption},
};

pub fn fread(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let fd = args.get_legacy_syscall_arg(1);
    let size = args.get_legacy_syscall_arg(2);
    let buffer_ptr = args.get_legacy_syscall_arg(3) as *mut u8;
    let pid = proc.pid();

    if size == 0 {
        proc.set_legacy_syscall_return(0, 0);
        return;
    }

    //keep for early retur (don't waste time on disk if buffer is invalid)
    if !syscall::verify_memory_range(buffer_ptr as u64, buffer_ptr as u64 + size) {
        proc.set_legacy_syscall_return(u64::MAX, 1);
        return;
    }

    let file_handle = {
        let proc_mut = proc.get_mutable();
        let namespaces = proc_mut.get_namespaces();
        let fs_namespace = namespaces
            .get_namespace::<crate::proc::FilesystemNamespace>(0)
            .expect("default fs namespace must exist");
        if let Some(f_handle) = fs_namespace.get_file_handle(fd) {
            f_handle
        } else {
            proc.set_legacy_syscall_return(u64::MAX, 1);
            return;
        }
    };

    let proc_clone = proc.downgrade();
    let task = async move {
        let proc = proc_clone;
        let f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = crate::memory::physical_allocator::allocate_contiguous(pages as u32);
        let buffers = (0..pages).map(|i| buffer_alloc + (i * 4096)).collect::<Vec<PhysAddr>>();

        let read_result = crate::vfs::read_file(f_handle.get(), &buffers, size).await;
        let Some(proc) = proc.upgrade() else {
            return; //proc was killed
        };
        let Ok(bytes_read) = read_result else {
            let proc_lock = proc.get();
            proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            return;
        };
        //copy to user buffer
        let dst = buffer_ptr as u64;
        let src: VirtAddr = buffer_alloc.into();
        let valid_copy = safe_memcpy_to_user(dst, src.0, size as usize);
        if !valid_copy {
            let proc_lock = proc.get();
            proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            return;
        }

        //free
        for i in 0..pages {
            unsafe { crate::memory::physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
        }

        //return
        proc.set_legacy_syscall_return(bytes_read, 0);
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}
