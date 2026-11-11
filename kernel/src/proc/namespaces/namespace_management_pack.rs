use std::{boxed::Box, sync::arc::Arc};

use crate::{
    memory::safe_memcpy_to_user,
    proc::{
        NamespaceHolder, ProcessData, get_namespace_id,
        namespaces::NamespaceType,
        syscall::{self, SyscallCpuState, SyscallHandler, SyscallPack},
    },
};

pub fn init_namespace_management_syscalls() {
    let handlers: [(SyscallHandler, &'static str); _] = [
        (mknamespace, "mknamespace"),
        (rmnamespace, "rmnamespace"),
        (chnamespace, "chnamespace"),
        (lsnamespace, "lsnamespace"),
    ];

    let namespace_management_syscalls = SyscallPack::new(Box::new(handlers));
    syscall::register_syscall_pack("namespace_management".into(), Arc::new(namespace_management_syscalls));
}

fn mknamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_type = args.get_arg(0);
    let existing_namespace_id = args.get_arg(1);
    let clear_namespace = args.get_arg(2) != 0;
    let Some(namespace_type) = NamespaceType::from_id(namespace_type) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let mutable = proc.get_mutable();
    let namespaces = mutable.get_namespaces();
    let existing_namespace = if existing_namespace_id == 0 {
        namespaces.get_default_by_type(namespace_type)
    } else {
        let Some(existing_namespace_) = namespaces.get_namespace_holder(existing_namespace_id) else {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        };
        existing_namespace_.clone()
    };
    drop(mutable);

    let namespace = if clear_namespace {
        let Ok(namespace) = NamespaceHolder::create_empty_from(get_namespace_id(), &existing_namespace) else {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        };
        namespace
    } else {
        let Ok(new_namespace) = NamespaceHolder::create_from(get_namespace_id(), &existing_namespace) else {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        };
        new_namespace
    };

    let id = namespace.get_id();

    proc.get_mutable().get_namespaces_mut().add_namespace(namespace);
    proc.set_syscall_return(&[id]);
}

fn rmnamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_arg(0);
    if proc.get_mutable().get_namespaces_mut().remove_namespace(namespace_id).is_ok() {
        proc.set_syscall_return(&[0]);
    } else {
        proc.set_syscall_return(&[u64::MAX]);
    }
}

fn chnamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_arg(0);
    if proc.get_mutable().get_namespaces_mut().change_namespace(namespace_id).is_ok() {
        proc.set_syscall_return(&[0]);
    } else {
        proc.set_syscall_return(&[u64::MAX]);
    }
}

#[repr(C)]
struct NamespaceInfo {
    id: u64,
    ns_type: NamespaceType,
    currently_used: bool,
}

fn lsnamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_buf_size = args.get_arg(0);
    let namespace_buf_ptr = args.get_arg(1);

    let mutable = proc.get_mutable();
    let namespaces = mutable.get_namespaces();

    for (i, ns) in namespaces.owned_namespaces.iter().enumerate() {
        if i as u64 >= namespace_buf_size {
            break;
        }
        let info = NamespaceInfo {
            id: ns.get_id(),
            ns_type: ns.get_type(),
            currently_used: namespaces.is_in_use(ns.get_id()),
        };
        let dst = namespace_buf_ptr + (i as u64 * std::mem::size_of::<NamespaceInfo>() as u64);
        let res = safe_memcpy_to_user(dst, (&raw const info) as u64, core::mem::size_of::<NamespaceInfo>());
        if !res {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    }

    let total_namespaces = namespaces.owned_namespaces.len() as u64;
    let written_namespaces = total_namespaces.min(namespace_buf_size);
    proc.set_syscall_return(&[written_namespaces, total_namespaces]);
}
