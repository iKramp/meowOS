use core::{
    fmt::Debug,
    sync::atomic::{AtomicBool, AtomicU64},
};
use std::{
    boxed::Box,
    lock_w_info,
    sync::no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
    vec::Vec,
};

use crate::{
    interrupts::InterruptProcessorState,
    memory::addresses::PhysAddr,
    proc::{ProcNamespaces, syscall::SyscallCpuState},
};

use super::Pid;

type ProcExitHook = Box<dyn FnOnce(u64) + Send>;

///Describes the process metadata like memory mapping, open files, etc.
pub struct ProcessData {
    pid: Pid,
    sleeping: AtomicBool,
    is_32_bit: bool,
    page_tree_root: AtomicU64,
    cmdline: Box<str>,
    internal: NoIntSpinlock<ProcessDataMutable>,
    exit_hooks: NoIntSpinlock<Vec<ProcExitHook>>,
}

impl Debug for ProcessData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessData")
            .field("pid", &self.pid)
            .field("sleeping", &self.sleeping)
            .field("is_32_bit", &self.is_32_bit)
            .field("page_tree_root", &self.page_tree_root)
            .field("cmdline", &self.cmdline)
            .field("internal", &self.internal)
            .finish()
    }
}

#[derive(Debug)]
pub struct ProcessDataMutable {
    cpu_state: CpuStateType,
    namespaces: ProcNamespaces,
}

impl ProcessDataMutable {
    pub fn get_namespaces(&self) -> &ProcNamespaces {
        &self.namespaces
    }

    pub(in crate::proc) fn get_namespaces_mut(&mut self) -> &mut ProcNamespaces {
        &mut self.namespaces
    }
}

#[derive(Debug)]
pub enum CpuStateType {
    Interrupt(InterruptProcessorState),
    Syscall(SyscallCpuState),
    None, //is currently running, was taken
}

pub enum StackCpuStateData<'a> {
    Interrupt(&'a InterruptProcessorState),
    Syscall(&'a SyscallCpuState),
}

impl ProcessData {
    pub(in crate::proc) fn new(
        pid: Pid,
        is_32_bit: bool,
        cmdline: Box<str>,
        cpu_state: CpuStateType,
        namespaces: ProcNamespaces,
    ) -> Self {
        let root = namespaces.memory_namespace.page_tree_root();
        Self {
            pid,
            sleeping: AtomicBool::new(false),
            is_32_bit,
            page_tree_root: AtomicU64::new(root.0),
            cmdline,
            internal: NoIntSpinlock::new(ProcessDataMutable { cpu_state, namespaces }),
            exit_hooks: NoIntSpinlock::new(Vec::new()),
        }
    }

    pub fn get_mutable<'a>(&'a self) -> NoIntSpinlockGuard<'a, ProcessDataMutable> {
        lock_w_info!(self.internal)
    }

    pub fn call_exit_hooks(&self, exit_code: u64) {
        let mut hooks = lock_w_info!(self.exit_hooks);
        for hook in hooks.drain(..) {
            hook(exit_code);
        }
    }

    pub fn add_exit_hook(&self, hook: Box<dyn FnOnce(u64) + Send>) {
        let mut hooks = lock_w_info!(self.exit_hooks);
        hooks.push(hook);
    }

    pub fn set_legacy_syscall_return(&self, val: u64, err: u64) {
        let internal = &mut lock_w_info!(self.internal);
        if let CpuStateType::Syscall(syscall_state) = &mut internal.cpu_state {
            syscall_state.rax = val;
            syscall_state.rdx = err;
        } else {
            panic!("set syscall return from non-syscall context: kill process");
        }
    }

    pub fn set_syscall_return(&self, values: &[u64]) {
        let internal = &mut lock_w_info!(self.internal);
        if let CpuStateType::Syscall(syscall_state) = &mut internal.cpu_state {
            if values.len() > 10 {
                panic!("too many return values for syscall");
            }

            let args_ptr = &raw mut syscall_state.rdx;
            for (i, &val) in values.iter().enumerate() {
                unsafe {
                    core::ptr::write_volatile(args_ptr.add(i), val);
                }
            }
        } else {
            panic!("set syscall return from non-syscall context: kill process");
        }
    }

    pub fn set_cpu_data(&self, cpu_state: CpuStateType) {
        let internal = &mut lock_w_info!(self.internal);
        internal.cpu_state = cpu_state;
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn page_tree(&self) -> PhysAddr {
        PhysAddr(self.page_tree_root.load(core::sync::atomic::Ordering::Acquire))
    }

    pub fn set_page_tree(&self, new_root: PhysAddr) {
        self.page_tree_root.store(new_root.0, core::sync::atomic::Ordering::Release);
    }

    pub fn take_cpu_state(&self) -> CpuStateType {
        let internal = &mut lock_w_info!(self.internal);
        core::mem::replace(&mut internal.cpu_state, CpuStateType::None)
    }

    pub fn set_sleeping(&self, sleeping: bool) {
        self.sleeping.store(sleeping, core::sync::atomic::Ordering::Release);
    }

    pub fn is_sleeping(&self) -> bool {
        self.sleeping.load(core::sync::atomic::Ordering::Acquire)
    }
}
