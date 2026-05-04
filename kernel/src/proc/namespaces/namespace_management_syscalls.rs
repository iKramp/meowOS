use std::{boxed::Box, sync::arc::Arc};

use crate::proc::syscall::{self, SyscallPack};

pub fn init_namespace_management_syscalls() {
    let handlers = [
        //todo
    ];

    let namespace_management_syscalls = SyscallPack::new(Box::new(handlers));
    syscall::register_syscall_pack("namespace_management".into(), Arc::new(namespace_management_syscalls));
}

pub fn mknamespace(_args: &mut syscall::SyscallArgs, _proc: &Arc<crate::proc::ProcessData>) -> bool {
    //todo
    false
}

pub fn rmnamespace(_args: &mut syscall::SyscallArgs, _proc: &Arc<crate::proc::ProcessData>) -> bool {
    //todo
    false
}

pub fn chnamespace(_args: &mut syscall::SyscallArgs, _proc: &Arc<crate::proc::ProcessData>) -> bool {
    //todo
    false
}

pub fn lsnamespace(_args: &mut syscall::SyscallArgs, _proc: &Arc<crate::proc::ProcessData>) -> bool {
    //todo
    false
}
