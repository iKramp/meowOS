use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    lock_w_info,
    mem_utils::PhysAddr,
    sync::no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
};

use crate::{
    interrupts::InterruptProcessorState,
    proc::{ProcNamespaces, syscall::SyscallCpuState},
    vfs::{
        InodeIdentifier,
        file::{FileDescriptor, FileHandle},
    },
};

use super::Pid;

///Describes the process metadata like memory mapping, open files, etc.
#[derive(Debug)]
pub struct ProcessData {
    pid: Pid,
    is_32_bit: bool,
    page_tree_root: PhysAddr,
    cmdline: Box<str>,
    internal: NoIntSpinlock<ProcessDataMutable>,
}

impl ProcessData {
    pub fn set_ret_status(&self, status: u64) {
        let internal = &mut lock_w_info!(self.internal);
        internal.return_status = Some(status);
    }
}

#[derive(Debug)]
pub struct ProcessDataMutable {
    cpu_state: CpuStateType,
    return_status: Option<u64>,
    file_handles: BTreeMap<u64, FileHandle>,
    file_handle_index: FileDescriptor,
    namespaces: ProcNamespaces,
}

impl ProcessDataMutable {
    pub(in crate::proc) fn get_namespaces(&self) -> &ProcNamespaces {
        &self.namespaces
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
            is_32_bit,
            page_tree_root: root,
            cmdline,
            internal: NoIntSpinlock::new(ProcessDataMutable {
                return_status: None,
                cpu_state,
                file_handles: BTreeMap::new(),
                file_handle_index: 0,
                namespaces,
            }),
        }
    }

    pub fn open_file_handle(&self, handle: FileHandle) -> FileDescriptor {
        let internal = &mut lock_w_info!(self.internal);
        let index = internal.file_handle_index;
        internal.file_handles.insert(index, handle);
        internal.file_handle_index += 1;
        index
    }

    pub fn get_inode(&self, fd: FileDescriptor) -> Option<InodeIdentifier> {
        let internal = lock_w_info!(self.internal);
        internal.file_handles.get(&fd).map(|handle| handle.inode)
    }

    pub fn get_mutable<'a>(&'a self) -> NoIntSpinlockGuard<'a, ProcessDataMutable> {
        lock_w_info!(self.internal)
    }

    pub fn set_syscall_return(&self, val: u64, err: u64) {
        let internal = &mut lock_w_info!(self.internal);
        if let CpuStateType::Syscall(syscall_state) = &mut internal.cpu_state {
            syscall_state.rax = val;
            syscall_state.rdx = err;
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
        self.page_tree_root
    }

    pub fn take_cpu_state(&self) -> CpuStateType {
        let internal = &mut lock_w_info!(self.internal);
        core::mem::replace(&mut internal.cpu_state, CpuStateType::None)
    }
}

impl ProcessDataMutable {
    pub fn get_file_handle(&self, fd: FileDescriptor) -> Option<&FileHandle> {
        self.file_handles.get(&fd)
    }

    pub fn take_file_handle(&mut self, fd: FileDescriptor) -> Option<FileHandle> {
        self.file_handles.remove(&fd)
    }

    pub fn insert_file_handle(&mut self, fd: FileDescriptor, handle: FileHandle) {
        self.file_handles.insert(fd, handle);
    }
}
