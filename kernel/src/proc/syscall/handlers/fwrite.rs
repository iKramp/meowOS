use std::{mem_utils::PhysAddr, println, sync::arc::Arc, vec::Vec};

use crate::{
    proc::{
        ProcessData,
        syscall::{self, SyscallCpuState},
    },
    task_runner::{self, PidOption},
};

pub fn fwrite(args: &mut SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let fd = args.get_legacy_syscall_arg(1);
    let size = args.get_legacy_syscall_arg(2);
    let buffer_ptr = args.get_legacy_syscall_arg(3) as *const u8;
    let proc = proc.clone();
    let pid = proc.pid();

    println!("fwrite called with: fd {}, size {}", fd, size);

    if !syscall::verify_memory_range(buffer_ptr as u64, buffer_ptr as u64 + size) {
        println!("fwrite: invalid buffer pointer or size");
        args.set_legacy_syscall_ret(u64::MAX, 1);
        return false;
    }

    if size == 0 {
        args.set_legacy_syscall_ret(0, 0);
        return true;
    }

    let file_handle = {
        let mut proc_mut = proc.get_mutable();
        if let Some(f_handle) = proc_mut.take_file_handle(fd) {
            f_handle
        } else {
            println!("fwrite: invalid fd {fd}");
            args.set_legacy_syscall_ret(u64::MAX, 1);
            return false;
        }
    };

    let task = async move {
        let mut f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = crate::memory::physical_allocator::allocate_contiguius_high(pages);
        let dst = std::mem_utils::translate_phys_virt_addr(buffer_alloc).0 as *mut u8;
        let src = buffer_ptr;
        //copy to user buffer
        unsafe { core::ptr::copy_nonoverlapping(src, dst, size as usize) };

        let buffers = (0..pages).map(|i| buffer_alloc + (i * 4096)).collect::<Vec<PhysAddr>>();

        let write_result = crate::vfs::write_file(&mut f_handle, &buffers, size).await;
        let Some(proc) = crate::proc::get_proc(proc.pid()) else {
            //free
            for i in 0..pages {
                unsafe { crate::memory::physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
            }
            return; //proc was killed
        };
        if write_result.is_err() {
            println!("fwrite: write failed");
            let proc_lock = proc.get();
            proc_lock.set_syscall_return(u64::MAX, 1);

            //free
            for i in 0..pages {
                unsafe { crate::memory::physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
            }
            return;
        }
        let bytes_written = unsafe { write_result.unwrap_unchecked() };

        //free
        for i in 0..pages {
            unsafe { crate::memory::physical_allocator::deallocate_frame(buffer_alloc + (i * 4096)) };
        }

        //return fd
        proc.get_mutable().insert_file_handle(fd, f_handle);

        //return
        proc.set_syscall_return(bytes_written, 0);
        println!("fwrite: finished processing fwrite for pid {pid:?}, bytes_written: {bytes_written}");
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    true
}
