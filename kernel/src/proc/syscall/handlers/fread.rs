use std::{mem_utils::PhysAddr, sync::arc::Arc, vec::Vec};

use crate::{
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

    if !syscall::verify_memory_range(buffer_ptr as u64, buffer_ptr as u64 + size) {
        proc.set_legacy_syscall_return(u64::MAX, 1);
        return false;
    }

    if size == 0 {
        proc.set_legacy_syscall_return(0, 0);
        return true;
    }

    let file_handle = {
        let mut proc_mut = proc.get_mutable();
        if let Some(f_handle) = proc_mut.take_file_handle(fd) {
            f_handle
        } else {
            proc.set_legacy_syscall_return(u64::MAX, 1);
            return false;
        }
    };

    let task = async move {
        let mut f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = crate::memory::physical_allocator::allocate_contiguius_high(pages);
        let buffers = (0..pages).map(|i| buffer_alloc + (i * 4096)).collect::<Vec<PhysAddr>>();

        let read_result = crate::vfs::read_file(&mut f_handle, &buffers, size).await;
        let Some(proc) = crate::proc::get_proc(proc.pid()) else {
            return; //proc was killed
        };
        let Ok(bytes_read) = read_result else {
            let proc_lock = proc.get();
            proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            return;
        };
        //copy to user buffer
        let dst = buffer_ptr;
        let src = std::mem_utils::translate_phys_virt_addr(buffer_alloc).0 as *const u8;
        unsafe { core::ptr::copy_nonoverlapping(src, dst, size as usize) };

        //free
        for i in 0..pages {
            unsafe { crate::memory::physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
        }

        //return fd
        proc.get_mutable().insert_file_handle(fd, f_handle);

        //return
        proc.set_legacy_syscall_return(bytes_read, 0);
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    true
}
