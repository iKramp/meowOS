use core::time::Duration;
use std::{
    boxed::Box,
    println,
    sync::arc::Arc,
    time::{FfiDuration, UNIX_EPOCH},
};

use crate::{
    memory::safe_memcpy_to_user,
    proc::{
        ProcessData, TimeNamespace,
        syscall::{SyscallCpuState, SyscallHandler, SyscallPack},
    },
};

pub(super) fn init_time_syscalls() {
    let handlers: [(SyscallHandler, &'static str); _] =
        [(time, "time"), (set_time_off, "set_time_off"), (set_time_off, "get_time_off")];

    let exec_syscall_pack = SyscallPack::new(Box::new(handlers));
    crate::proc::syscall::register_syscall_pack("time".into(), Arc::new(exec_syscall_pack));
}

#[repr(C)]
struct TimeInfo {
    time_since_epoch: FfiDuration,
    time_since_boot: FfiDuration,
    program_start_time: FfiDuration,
    program_userspace_cpu_time: FfiDuration,
    program_kernelspace_cpu_time: FfiDuration,
}

fn time(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let now = std::time::Instant::now();
    let mut time_since_epoch = now.duration_since(UNIX_EPOCH);
    let mut time_since_boot = now.duration_since(crate::clocks::get_time_of_boot());
    let mut program_start_time_since_epoch = proc.get_start_time().duration_since(UNIX_EPOCH);
    let program_userspace_cpu_time = now.duration_since(UNIX_EPOCH);
    let program_kernelspace_cpu_time = now.duration_since(UNIX_EPOCH);

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let time_namespace = namespaces
        .get_namespace::<TimeNamespace>(namespace_id)
        .expect("Time namespace must exist");

    let (monotonic_offset, boot_time_offset) = time_namespace.get_offsets();
    let monotonic_negative = monotonic_offset < 0;
    let boot_time_negative = boot_time_offset < 0;
    let monotonic_offset = Duration::from_nanos_u128(monotonic_offset.unsigned_abs());
    let boot_time_offset = Duration::from_nanos_u128(boot_time_offset.unsigned_abs());

    drop(proc_mut);

    if boot_time_negative {
        time_since_boot = time_since_boot.saturating_sub(boot_time_offset);
    } else {
        time_since_boot = time_since_boot.saturating_add(boot_time_offset);
    }
    if monotonic_negative {
        time_since_epoch = time_since_epoch.saturating_sub(monotonic_offset);
        program_start_time_since_epoch = program_start_time_since_epoch.saturating_sub(monotonic_offset);
    } else {
        time_since_epoch = time_since_epoch.saturating_add(monotonic_offset);
        program_start_time_since_epoch = program_start_time_since_epoch.saturating_add(monotonic_offset);
    }
    let boot_time = time_since_epoch.saturating_sub(time_since_boot);
    if program_start_time_since_epoch < boot_time {
        program_start_time_since_epoch = boot_time;
    }

    let time_info = TimeInfo {
        time_since_epoch: FfiDuration::from(time_since_epoch),
        time_since_boot: FfiDuration::from(time_since_boot),
        program_start_time: FfiDuration::from(program_start_time_since_epoch),
        program_userspace_cpu_time: FfiDuration::from(program_userspace_cpu_time),
        program_kernelspace_cpu_time: FfiDuration::from(program_kernelspace_cpu_time),
    };

    let valid = safe_memcpy_to_user(
        args.get_arg(0),
        &time_info as *const TimeInfo as u64,
        core::mem::size_of::<TimeInfo>(),
    );
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
    } else {
        proc.set_syscall_return(&[0]);
    }
}

fn set_time_off(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let monotonic_high = args.get_arg(0) as i128;
    let monotonic_low = args.get_arg(1) as i128;
    let boot_high = args.get_arg(2) as i128;
    let boot_low = args.get_arg(3) as i128;

    let monotonic_offset = (monotonic_high << 64) | monotonic_low;
    let boot_offset = (boot_high << 64) | boot_low;

    let proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces();
    let time_namespace = namespaces
        .get_namespace::<TimeNamespace>(namespace_id)
        .expect("Time namespace must exist");
    drop(proc_mut);

    time_namespace.apply_offset(monotonic_offset, boot_offset);
    proc.set_syscall_return(&[0]);
}
