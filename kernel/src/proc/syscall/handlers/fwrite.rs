use std::{println, sync::arc::Arc, vec::Vec};

use crate::{
    memory::{addresses::*, safe_memcpy_from_user},
    proc::{
        ProcessData,
        syscall::{self, SyscallCpuState},
    },
    task_runner::{self, PidOption},
};

pub fn fwrite(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let fd = args.get_legacy_syscall_arg(1);
    let size = args.get_legacy_syscall_arg(2);
    let buffer_ptr = args.get_legacy_syscall_arg(3) as *const u8;
    let pid = proc.pid();

    if size == 0 {
        proc.set_legacy_syscall_return(0, 0);
        return;
    }

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
            println!("fwrite: invalid fd {fd}");
            proc.set_legacy_syscall_return(u64::MAX, 1);
            return;
        }
    };

    let proc_clone = proc.clone();
    let task = async move {
        let proc = proc_clone;
        let f_handle = file_handle; //get to local
        let pages = size.div_ceil(4096);
        let buffer_alloc = crate::memory::physical_allocator::allocate_contiguous(pages as u32);
        let dst = VirtAddr::from(buffer_alloc.0.start).0 as *mut u8;
        let src = buffer_ptr;
        //copy to user buffer
        let copy_valid = safe_memcpy_from_user(dst as u64, src as u64, size as usize);
        if !copy_valid {
            let proc_lock = proc.get();
            proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            return;
        }

        let buffers = buffer_alloc.get_range().get_addresses().collect::<Vec<PhysAddr>>();
        let proc_weak = proc.downgrade();
        drop(proc);

        let write_result = crate::vfs::write_file(f_handle.get(), &buffers, size).await;

        let Some(proc) = proc_weak.upgrade() else {
            return; //proc was killed
        };
        if write_result.is_err() {
            println!("fwrite: write failed");
            let proc_lock = proc.get();
            proc_lock.set_legacy_syscall_return(u64::MAX, 1);
            return;
        }
        let bytes_written = unsafe { write_result.unwrap_unchecked() };

        //return
        proc.set_legacy_syscall_return(bytes_written, 0);
        println!("fwrite: finished processing fwrite for pid {pid:?}, bytes_written: {bytes_written}");
        crate::proc::wake_process(proc.pid())
    };

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);

    task_runner::add_task(ffi_safe_task, PidOption::Some(pid));
    proc.set_sleeping(true);
}
