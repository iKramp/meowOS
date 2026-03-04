use std::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    lock_w_info,
    sync::{
        arc::Arc,
        no_int_spinlock::{NoIntSpinlock, NoIntSpinlockGuard},
    },
};

use crate::{
    interrupts::InterruptProcessorState,
    memory::paging::PageTree,
    vfs::{
        InodeIdentifier,
        file::{FileDescriptor, FileHandle},
    },
};

use super::{MemoryContext, Pid, syscall::SyscallCpuState};

///Describes the process metadata like memory mapping, open files, etc.
#[derive(Debug)]
pub struct ProcessData {
    pid: Pid,
    is_32_bit: bool,
    cmdline: Box<str>,
    page_tree_root: PageTree,
    internal: NoIntSpinlock<ProcessDataMutable>,
}

impl ProcessData {
    pub fn cleanup(&self) {
        let mut internal = lock_w_info!(self.internal);
        internal.memory_context = Arc::new(MemoryContext::default());
        internal.file_handles.clear();
    }

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
    memory_context: Arc<MemoryContext>,
}

#[derive(Debug)]
pub enum CpuStateType {
    Interrupt(InterruptProcessorState),
    Syscall((SyscallCpuState, u64)), //cpu state + userspace stack pointer
    None,                            //is currently running, was taken
}

pub enum StackCpuStateData<'a> {
    Interrupt(&'a InterruptProcessorState),
    Syscall(&'a SyscallCpuState),
}

impl ProcessData {
    pub fn new(
        pid: Pid,
        is_32_bit: bool,
        cmdline: Box<str>,
        memory_context: Arc<MemoryContext>,
        cpu_state: CpuStateType,
    ) -> Self {
        Self {
            pid,
            is_32_bit,
            cmdline,
            page_tree_root: memory_context.page_tree.clone(),
            internal: NoIntSpinlock::new(ProcessDataMutable {
                return_status: None,
                memory_context,
                cpu_state,
                file_handles: BTreeMap::new(),
                file_handle_index: 0,
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
        if let CpuStateType::Syscall((syscall_state, _)) = &mut internal.cpu_state {
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

    pub fn page_tree(&self) -> &PageTree {
        &self.page_tree_root
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
