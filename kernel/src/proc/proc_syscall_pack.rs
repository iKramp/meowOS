use std::{boxed::Box, error::KernelError, lock_w_info, println, string::ToString, sync::arc::Arc};

use crate::{
    clocks::{ScheduledEvent, schedule_event},
    interrupts::InterruptProcessorState,
    memory::{addresses::VirtAddr, safe_memcpy_from_user},
    proc::{
        self, NamespaceIds, PROCESS_ID_COUNTER, Pid, ProcNamespaces, ProcessData, SCHEDULER,
        process_data::CpuStateType,
        signal::SignalType,
        syscall::{SyscallCpuState, SyscallHandler, SyscallPack},
    },
};

pub(super) fn init_proc_syscalls() {
    let handlers: [(SyscallHandler, &'static str); _] = [
        (exec, "exec"),
        (exit, "exit"),
        (sleep, "sleep"),
        (get_pid, "get_pid"),
        (set_sig_handler, "set_sig_handler"),
        (remove_sig_handler, "remove_sig_handler"),
        (send_signal, "send_signal"),
    ];

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

#[derive(Debug, Clone)]
#[repr(C)]
struct ExecArgs {
    namespaces: NamespaceIds,
    registers: RegisterState,
    start_ptr: u64,
    name_len: u64,
    name_ptr: u64,
    clone_ns_table: bool,
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

    println!("exec called with {:#x?} for process {:?}", exec_args, proc.pid());

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
        .clone_from_ids(exec_args.namespaces.clone(), exec_args.clone_ns_table)
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
) -> Result<Pid, KernelError> {
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

fn get_pid(_args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let pid = proc.pid();
    proc.set_syscall_return(&[pid.0 as u64]);
}

fn set_sig_handler(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let signal_number = args.get_arg(0) as usize;
    let handler_ptr = args.get_arg(1);

    let Ok(sig) = SignalType::try_from(signal_number) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    proc.get_mutable()
        .get_signal_state_mut()
        .set_handler(sig, VirtAddr(handler_ptr));
    proc.set_syscall_return(&[0]);
}

fn remove_sig_handler(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let signal_number = args.get_arg(0) as usize;

    let Ok(sig) = SignalType::try_from(signal_number) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    proc.get_mutable().get_signal_state_mut().remove_handler(sig);
    proc.set_syscall_return(&[0]);
}

fn send_signal(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let target_pid = Pid(args.get_arg(0) as u32);
    let signal_number = args.get_arg(1) as usize;
    let data_len = args.get_arg(2) as usize;
    let data_ptr = args.get_arg(3);

    if data_len > 128 {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }

    let data_buf_uninit = Box::new_uninit_slice(data_len);
    let valid = safe_memcpy_from_user(data_buf_uninit.as_ptr() as u64, data_ptr, data_len);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }
    let data_buf = unsafe { data_buf_uninit.assume_init() };

    let Ok(sig) = SignalType::try_from(signal_number) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let target_proc = match proc::get_proc(target_pid) {
        Some(proc) => proc,
        None => {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    };

    let custom_signal_data = proc::signal::CustomSignalFields::new(data_buf);

    let signal_data = proc::signal::SignalData {
        signal_type: sig,
        source: proc.pid(),
        fields: Box::new(custom_signal_data),
    };

    target_proc
        .get_mutable()
        .get_signal_state_mut()
        .send_signal(signal_data, target_proc.pid());
    proc.set_syscall_return(&[0]);
}
