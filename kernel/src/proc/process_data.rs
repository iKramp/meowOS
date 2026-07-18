use core::sync::atomic::{AtomicBool, AtomicU64};
use std::{
    boxed::Box,
    lock_w_info,
    sync::no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
};

use crate::{
    interrupts::InterruptProcessorState,
    memory::addresses::PhysAddr,
    proc::{ProcNamespaces, syscall::SyscallCpuState},
};

use super::Pid;

///Describes the process metadata like memory mapping, open files, etc.
#[derive(Debug)]
pub struct ProcessData {
    pid: Pid,
    sleeping: AtomicBool,
    is_32_bit: bool,
    page_tree_root: AtomicU64,
    cmdline: Box<str>,
    internal: NoIntSpinlock<ProcessDataMutable>,
}

#[derive(Debug)]
pub struct ProcessDataMutable {
    cpu_state: CpuStateType,
    return_status: Option<u64>,
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
            internal: NoIntSpinlock::new(ProcessDataMutable {
                return_status: None,
                cpu_state,
                namespaces,
            }),
        }
    }

    pub fn get_mutable<'a>(&'a self) -> NoIntSpinlockGuard<'a, ProcessDataMutable> {
        lock_w_info!(self.internal)
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

    pub fn set_exit_status(&self, status: u64) {
        let internal = &mut lock_w_info!(self.internal);
        internal.return_status = Some(status);
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
