use std::boxed::Box;
use std::sync::arc::Arc;

use bitfield::bitfield;

use crate::proc::syscall::{SyscallCpuState, SyscallPack, register_syscall_pack};
use crate::proc::{ProcessData, syscall};

pub(super) fn init_fs_syscall_pack() {
    let handlers = [fopen, fclose, fread, fwrite, fseek];

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

bitfield! {
    struct OpenFlags(u64);
    impl Debug;
    pub read, set_read: 0;
    pub write, set_write: 1;
    pub append, set_append: 2;
    pub truncate, set_truncate: 3;
}

fn fopen(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let path_len = args.get_arg(0) as usize;
    let path_ptr = args.get_arg(1);
    let flags = OpenFlags(args.get_arg(2));

    let valid = syscall::verify_memory_range(path_ptr, path_ptr + path_len as u64);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }
    let Ok(path) = core::str::from_utf8(unsafe { core::slice::from_raw_parts(path_ptr as *const u8, path_len) }) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    todo!("implement fopen");
}

fn fclose(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);

    todo!("implement fclose");
}

fn fread(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let count = args.get_arg(1);
    let buf_ptr = args.get_arg(2);

    let valid = syscall::verify_memory_range(buf_ptr, buf_ptr + count);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    todo!("implement fread");
}

fn fwrite(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let count = args.get_arg(1);
    let buf_ptr = args.get_arg(2);

    let valid = syscall::verify_memory_range(buf_ptr, buf_ptr + count);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    todo!("implement fwrite");
}

fn fseek(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let fd = args.get_arg(0);
    let offset = args.get_arg(1) as i64;
    let mode_value = args.get_arg(2);
    let Some(mode) = FileSeekMode::from_u64(mode_value) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    todo!("implement fseek");
}
