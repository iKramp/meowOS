use std::{boxed::Box, sync::arc::Arc};

use crate::{
    memory::safe_memcpy_to_user,
    proc::{
        ProcessData, SyscallNamespace,
        syscall::{self, SyscallCpuState, SyscallPack, syscall_registry},
    },
};

pub fn init_syscall_management_syscalls() {
    let handlers = [lsgroups, lsallgroups, map_group, unmap_group, restrict];

    let syscall_management_syscalls = SyscallPack::new(Box::new(handlers));
    syscall::register_syscall_pack("syscall_management".into(), Arc::new(syscall_management_syscalls));
}

#[repr(C)]
struct GroupInfo {
    name_len: u8,
    name: [u8; 31],
}

#[repr(C)]
struct MappedGroupInfo {
    group_info: GroupInfo,
    offset: u32,
    mask: u32,
}

fn lsgroups(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let groups_buf_ptr = args.get_arg(0);
    let groups_buf_size = args.get_arg(1);

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_namespace::<SyscallNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };
    let mapped_syscalls = syscall_namespace.get_mapped_syscalls();
    let names = syscall_registry::get_names(mapped_syscalls.iter().map(|mapped| mapped.2));

    let to_write = mapped_syscalls.iter().zip(names).enumerate();
    for (i, ((mapped_base, mapped_mask, _mapped_id), name)) in to_write {
        if i as u64 >= groups_buf_size {
            break;
        }

        let group_info = MappedGroupInfo {
            group_info: GroupInfo {
                name_len: name.len() as u8,
                name: {
                    let mut name_arr = [0u8; 31];
                    let bytes = name.as_bytes();
                    let copy_len = bytes.len().min(31);
                    name_arr[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    name_arr
                },
            },
            offset: *mapped_base,
            mask: *mapped_mask,
        };

        let dest_ptr = groups_buf_ptr + i as u64 * core::mem::size_of::<MappedGroupInfo>() as u64;
        let res = safe_memcpy_to_user(
            dest_ptr,
            (&raw const group_info) as u64,
            core::mem::size_of::<MappedGroupInfo>(),
        );
        if !res {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    }

    let total_groups = mapped_syscalls.len() as u64;
    let written_groups = total_groups.min(groups_buf_size);
    proc.set_syscall_return(&[written_groups, total_groups]);
}

fn lsallgroups(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let groups_buf_ptr = args.get_arg(0);
    let groups_buf_size = args.get_arg(1);

    let all_packs = syscall_registry::get_all_pack_info();
    let to_write = all_packs.iter().enumerate();
    for (i, (name, _)) in to_write {
        if i as u64 >= groups_buf_size {
            break;
        }

        let group_info = GroupInfo {
            name_len: name.len() as u8,
            name: {
                let mut name_arr = [0u8; 31];
                let bytes = name.as_bytes();
                let copy_len = bytes.len().min(31);
                name_arr[..copy_len].copy_from_slice(&bytes[..copy_len]);
                name_arr
            },
        };

        let dest_ptr = groups_buf_ptr + i as u64 * core::mem::size_of::<GroupInfo>() as u64;
        let res = safe_memcpy_to_user(dest_ptr, (&raw const group_info) as u64, core::mem::size_of::<GroupInfo>());
        if !res {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    }

    let total_groups = all_packs.len() as u64;
    let written_groups = total_groups.min(groups_buf_size);
    proc.set_syscall_return(&[written_groups, total_groups]);
}

fn map_group(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let group_name_len = args.get_arg(0);
    let group_name_ptr = args.get_arg(1);
    let base_index = args.get_arg(2);

    let Some(group_name) = syscall::string_from_args(group_name_ptr, group_name_len) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let (pack, pack_id) = match syscall_registry::get_syscall_pack(&group_name) {
        Some(info) => info,
        None => {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    };

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_namespace::<SyscallNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    syscall_namespace.map_syscall_pack(base_index as u32, pack, pack_id);

    proc.set_syscall_return(&[0]);
}

fn unmap_group(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let offset = args.get_arg(0);

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_namespace::<SyscallNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let res = syscall_namespace.unmap_syscall_pack_by_offset(offset);
    if res.is_err() {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }

    proc.set_syscall_return(&[0]);
}

fn restrict(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let offset = args.get_arg(0);
    let mask = args.get_arg(1);

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_namespace::<SyscallNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    let res = syscall_namespace.disable_syscall_by_mask(offset as u32, mask as u32);
    if res.is_err() {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    proc.set_syscall_return(&[0]);
}
