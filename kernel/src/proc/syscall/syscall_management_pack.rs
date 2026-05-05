use std::{boxed::Box, sync::arc::Arc};

use crate::proc::{
    ProcessData,
    syscall::{self, SyscallCpuState, SyscallPack, syscall_registry},
};

pub fn init_syscall_management_syscalls() {
    let handlers = [lsgroups, lsallgroups];

    let syscall_management_syscalls = SyscallPack::new(Box::new(handlers));
    syscall::register_syscall_pack("syscall_management".into(), Arc::new(syscall_management_syscalls));
}

#[repr(C)]
struct MappedGroupInfo {
    name_len: u8,
    name: [u8; 31],
    offset: u32,
    mask: u32,
}

fn lsgroups(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let groups_buf_ptr = args.get_arg(0);
    let groups_buf_size = args.get_arg(1);

    let valid = syscall::verify_memory_range(
        groups_buf_ptr,
        groups_buf_ptr + groups_buf_size * core::mem::size_of::<MappedGroupInfo>() as u64,
    );
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_syscall_namespace(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };
    let mapped_syscalls = syscall_namespace.get_mapped_syscalls();
    let names = syscall_registry::get_names(mapped_syscalls.iter().map(|mapped| mapped.2));

    let to_write = mapped_syscalls.iter().zip(names).enumerate();
    for (i, ((mapped_base, mapped_mask, _mapped_id), name)) in to_write {
        if i as u64 >= groups_buf_size {
            break;
        }

        let group_info = MappedGroupInfo {
            name_len: name.len() as u8,
            name: {
                let mut name_arr = [0u8; 31];
                let bytes = name.as_bytes();
                let copy_len = bytes.len().min(31);
                name_arr[..copy_len].copy_from_slice(&bytes[..copy_len]);
                name_arr
            },
            offset: *mapped_base,
            mask: *mapped_mask,
        };

        unsafe {
            let dest_ptr = (groups_buf_ptr + i as u64 * core::mem::size_of::<MappedGroupInfo>() as u64) as *mut MappedGroupInfo;
            dest_ptr.write(group_info);
        }
    }

    let total_groups = mapped_syscalls.len() as u64;
    let written_groups = total_groups.min(groups_buf_size);
    proc.set_syscall_return(&[written_groups, total_groups]);

    false
}

#[repr(C)]
struct GroupInfo {
    name_len: u8,
    name: [u8; 31],
    syscall_count: u8,
}

fn lsallgroups(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let groups_buf_ptr = args.get_arg(0);
    let groups_buf_size = args.get_arg(1);

    let valid = syscall::verify_memory_range(
        groups_buf_ptr,
        groups_buf_ptr + groups_buf_size * core::mem::size_of::<GroupInfo>() as u64,
    );
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    let all_packs = syscall_registry::get_all_pack_info();
    let to_write = all_packs.iter().enumerate();
    for (i, (name, syscall_count)) in to_write {
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
            syscall_count: *syscall_count,
        };

        unsafe {
            let dest_ptr = (groups_buf_ptr + i as u64 * core::mem::size_of::<GroupInfo>() as u64) as *mut GroupInfo;
            dest_ptr.write(group_info);
        }
    }

    let total_groups = all_packs.len() as u64;
    let written_groups = total_groups.min(groups_buf_size);
    proc.set_syscall_return(&[written_groups, total_groups]);

    false
}

fn mapgroup(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let group_name_len = args.get_arg(0);
    let group_name_ptr = args.get_arg(1);
    let base_index = args.get_arg(2);

    let valid = syscall::verify_memory_range(group_name_ptr, group_name_ptr + group_name_len);
    if !valid {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    }

    let group_name = unsafe {
        let slice = core::slice::from_raw_parts(group_name_ptr as *const u8, group_name_len as usize);
        core::str::from_utf8_unchecked(slice) //don't care if it's valid, bytes just have to match
    };

    let (pack, pack_id) = match syscall_registry::get_syscall_pack(group_name) {
        Some(info) => info,
        None => {
            proc.set_syscall_return(&[u64::MAX]);
            return false;
        }
    };

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_syscall_namespace(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    syscall_namespace.map_syscall_pack(base_index as u32, pack, pack_id);

    proc.set_syscall_return(&[0]);
    false
}

