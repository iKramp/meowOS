use std::{boxed::Box, error::ErrorCode, lock_w_info, string::ToString, sync::arc::Arc};

use crate::{
    acpi::{ScheduledEvent, schedule_event},
    interrupts::InterruptProcessorState,
    memory::safe_memcpy_from_user,
    proc::{
        self, NamespaceIds, PROCESS_ID_COUNTER, Pid, ProcNamespaces, ProcessData, SCHEDULER,
        process_data::CpuStateType,
        syscall::{SyscallCpuState, SyscallHandler, SyscallPack},
    },
};

pub fn init_proc_syscalls() {
    let handlers: [SyscallHandler; _] = [exec, exit, sleep];

    let exec_syscall_pack = SyscallPack::new(Box::new(handlers));
    crate::proc::syscall::register_syscall_pack("proc".into(), Arc::new(exec_syscall_pack));
}

#[cfg(target_arch = "x86_64")]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RegisterState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

#[repr(C)]
struct ExecArgs {
    namespaces: NamespaceIds,
    registers: RegisterState,
    start_ptr: u64,
    name_len: u64,
    name_ptr: u64,
}

fn exec(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let exec_args_ptr = args.get_arg(0);

    let exec_args_heap_box = Box::<ExecArgs>::new_uninit();
    let exec_args_heap_ptr = exec_args_heap_box.as_ptr() as u64;

    let valid = safe_memcpy_from_user(exec_args_heap_ptr, exec_args_ptr, core::mem::size_of::<ExecArgs>());
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }
    let exec_args = unsafe { exec_args_heap_box.assume_init() };

    let name_buf_uninit = Box::new_uninit_slice(exec_args.name_len as usize);
    let valid = safe_memcpy_from_user(
        name_buf_uninit.as_ptr() as u64,
        exec_args.name_ptr,
        exec_args.name_len as usize,
    );
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }
    let name_buf = unsafe { name_buf_uninit.assume_init() };

    let name = unsafe {
        let res = core::str::from_utf8(&name_buf);
        if res.is_err() {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
        res.unwrap_unchecked()
    };

    let Ok(namespaces) = proc
        .get_mutable()
        .get_namespaces()
        .clone_from_ids(exec_args.namespaces.clone())
    else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let Ok(pid) = create_process_from_parts(exec_args.registers, exec_args.start_ptr, namespaces, name) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    proc.set_syscall_return(&[pid.0 as u64]);
}

pub fn create_process_from_parts(
    register_states: RegisterState,
    start_ptr: u64,
    namespaces: ProcNamespaces,
    name: &str,
) -> Result<Pid, ErrorCode> {
    let cpu_state = InterruptProcessorState::new_full(
        register_states.r15,
        register_states.r14,
        register_states.r13,
        register_states.r12,
        register_states.r11,
        register_states.r10,
        register_states.r9,
        register_states.r8,
        register_states.rbp,
        register_states.rsp,
        register_states.rdi,
        register_states.rsi,
        register_states.rdx,
        register_states.rcx,
        register_states.rbx,
        register_states.rax,
        start_ptr,
    );

    let pid = Pid(PROCESS_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed));
    let process_data = ProcessData::new(
        pid,
        false,
        name.to_string().into_boxed_str(),
        CpuStateType::Interrupt(cpu_state),
        namespaces,
    );

    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.accept_new_process(pid, process_data);

    Ok(pid)
}

fn exit(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let status = args.get_arg(0);
    proc::kill_process(proc.pid(), status);
}

fn sleep(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let sleep_time_sec = args.get_arg(0);
    let sleep_time_ns = args.get_arg(1);
    let sleep_time = core::time::Duration::new(sleep_time_sec, sleep_time_ns as u32);
    let sleep_until = std::time::Instant::now() + sleep_time;

    let pid = proc.pid();

    let scheduled_event = ScheduledEvent {
        time: sleep_until,
        callback: Box::new(move || {
            proc::wake_process(pid);
        }),
    };

    proc.set_sleeping(true);

    schedule_event(scheduled_event);
}
