use std::{mem_utils::PhysAddr, sync::arc::Arc, vec::Vec};

use crate::{
    memory::safe_memcpy_to_user,
    proc::{
        ProcessData,
        syscall::{self, SyscallCpuState},
    },
    task_runner::{self, PidOption},
};

pub fn fread(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let fd = args.get_legacy_syscall_arg(1);
    let size = args.get_legacy_syscall_arg(2);
    let buffer_ptr = args.get_legacy_syscall_arg(3) as *mut u8;
    let proc = proc.clone();
    let pid = proc.pid();

    if size == 0 {
        proc.set_legacy_syscall_return(0, 0);
        return true;
    }

    //keep for early retur (don't waste time on disk if buffer is invalid)
    if !syscall::verify_memory_range(buffer_ptr as u64, buffer_ptr as u64 + size) {
        proc.set_legacy_syscall_return(u64::MAX, 1);
        return false;
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
            return false;
        }
    };

    let task = async move {
        let f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = crate::memory::physical_allocator::allocate_contiguius_high(pages);
        let buffers = (0..pages).map(|i| buffer_alloc + (i * 4096)).collect::<Vec<PhysAddr>>();

        let read_result = crate::vfs::read_file(f_handle.get(), &buffers, size).await;
        let Some(proc) = crate::proc::get_proc(proc.pid()) else {
            return; //proc was killed
        };
        let Ok(bytes_read) = read_result else {
            let proc_lock = proc.get();
            proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            return;
        };
        //copy to user buffer
        let dst = buffer_ptr as u64;
        let src = std::mem_utils::translate_phys_virt_addr(buffer_alloc).0;
        let valid_copy = safe_memcpy_to_user(dst, src, size as usize);
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
    true
}
