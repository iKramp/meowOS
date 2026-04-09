use std::{boxed::Box, sync::arc::Arc};

use crate::proc::syscall::{self, SyscallPack, syscall_registry};

pub fn init_legacy_syscalls() {
    let handlers = [
        syscall::handlers::illegal,
        syscall::handlers::exit,
        syscall::handlers::illegal,
        syscall::handlers::illegal,
        syscall::handlers::fopen,
        syscall::handlers::fclose,
        syscall::handlers::fread,
        syscall::handlers::fwrite,
        syscall::handlers::illegal,
        syscall::handlers::illegal,
        syscall::handlers::illegal,
        syscall::handlers::illegal,
        syscall::handlers::time,
    ];

    let legacy_syscalls = SyscallPack::new(Box::new(handlers));
    syscall_registry::register_syscall_pack("legacy".into(), Arc::new(legacy_syscalls));
}
