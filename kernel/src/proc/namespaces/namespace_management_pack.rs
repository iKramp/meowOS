use std::{boxed::Box, sync::arc::Arc};

use crate::{
    memory::safe_memcpy_to_user,
    proc::{
        NamespaceHolder, ProcessData, get_namespace_id,
        namespaces::NamespaceType,
        syscall::{self, SyscallCpuState, SyscallPack},
    },
};

pub fn init_namespace_management_syscalls() {
    let handlers = [mknamespace, rmnamespace, chnamespace, lsnamespace];

    let namespace_management_syscalls = SyscallPack::new(Box::new(handlers));
    syscall::register_syscall_pack("namespace_management".into(), Arc::new(namespace_management_syscalls));
}

fn mknamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_type = args.get_arg(0);
    let existing_namespace = args.get_arg(1);
    let Some(namespace_type) = NamespaceType::from_id(namespace_type) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let namespace;

    if existing_namespace == 0 {
        let Some(namespace_) = namespace_type.create_empty_namespace(get_namespace_id()) else {
            proc.set_syscall_return(&[u64::MAX]);
            return false;
        };
        namespace = namespace_;
    } else {
        let mutable = proc.get_mutable();
        let namespaces = mutable.get_namespaces();
        let Some(existing_namespace) = namespaces.get_namespace_holder(existing_namespace) else {
            proc.set_syscall_return(&[u64::MAX]);
            return false;
        };
        let Ok(new_namespace) = NamespaceHolder::create_from(get_namespace_id(), existing_namespace) else {
            proc.set_syscall_return(&[u64::MAX]);
            return false;
        };
        namespace = new_namespace;
    }

    proc.get_mutable().get_namespaces_mut().add_namespace(namespace);
    proc.set_syscall_return(&[0]);
    false
}

fn rmnamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_arg(0);
    if proc.get_mutable().get_namespaces_mut().remove_namespace(namespace_id).is_ok() {
        proc.set_syscall_return(&[0]);
    } else {
        proc.set_syscall_return(&[u64::MAX]);
    }
    false
}

fn chnamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_arg(0);
    if proc.get_mutable().get_namespaces_mut().change_namespace(namespace_id).is_ok() {
        proc.set_syscall_return(&[0]);
    } else {
        proc.set_syscall_return(&[u64::MAX]);
    }
    false
}

#[repr(C)]
struct NamespaceInfo {
    id: u64,
    ns_type: NamespaceType,
    currently_used: bool,
}

fn lsnamespace(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_buf_ptr = args.get_arg(0);
    let namespace_buf_size = args.get_arg(1);

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
            return false;
        }
    }

    let total_namespaces = namespaces.owned_namespaces.len() as u64;
    let written_namespaces = total_namespaces.min(namespace_buf_size);
    proc.set_syscall_return(&[written_namespaces, total_namespaces]);
    false
}
