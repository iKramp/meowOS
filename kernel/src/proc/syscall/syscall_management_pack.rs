use std::{boxed::Box, println, sync::arc::Arc};

use crate::{
    memory::safe_memcpy_to_user,
    proc::{
        ProcessData, SyscallNamespace,
        syscall::{self, SyscallCpuState, SyscallPack, syscall_registry},
    },
};

pub fn init_syscall_management_syscalls() {
    let handlers = [lspacks, lsallpacks, map_pack, unmap_pack, restrict];

    let syscall_management_syscalls = SyscallPack::new(Box::new(handlers));
    syscall::register_syscall_pack("syscall_management".into(), Arc::new(syscall_management_syscalls));
}

#[repr(C)]
struct PackInfo {
    name_len: u8,
    name: [u8; 31],
}

#[repr(C)]
struct MappedPackInfo {
    pack_info: PackInfo,
    offset: u32,
    mask: u32,
}

fn lspacks(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let packs_buf_size = args.get_arg(0);
    let packs_buf_ptr = args.get_arg(1);

    let proc_mutable = proc.get_mutable();
    let Some(syscall_namespace) = proc_mutable.get_namespaces().get_namespace::<SyscallNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };
    drop(proc_mutable);
    let mapped_syscalls = syscall_namespace.get_mapped_syscalls();
    let names = syscall_registry::get_names(mapped_syscalls.iter().map(|mapped| mapped.2));

    let to_write = mapped_syscalls.iter().zip(names).enumerate();
    for (i, ((mapped_base, mapped_mask, _mapped_id), name)) in to_write {
        if i as u64 >= packs_buf_size {
            break;
        }

        let pack_info = MappedPackInfo {
            pack_info: PackInfo {
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

        let dest_ptr = packs_buf_ptr + i as u64 * core::mem::size_of::<MappedPackInfo>() as u64;
        let res = safe_memcpy_to_user(
            dest_ptr,
            (&raw const pack_info) as u64,
            core::mem::size_of::<MappedPackInfo>(),
        );
        if !res {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    }

    let total_packs = mapped_syscalls.len() as u64;
    let written_packs = total_packs.min(packs_buf_size);
    proc.set_syscall_return(&[written_packs, total_packs]);
}

fn lsallpacks(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let packs_buf_size = args.get_arg(0);
    let packs_buf_ptr = args.get_arg(1);

    let all_packs = syscall_registry::get_all_pack_info();
    let to_write = all_packs.iter().enumerate();
    for (i, (name, _)) in to_write {
        if i as u64 >= packs_buf_size {
            break;
        }

        let pack_info = PackInfo {
            name_len: name.len() as u8,
            name: {
                let mut name_arr = [0u8; 31];
                let bytes = name.as_bytes();
                let copy_len = bytes.len().min(31);
                name_arr[..copy_len].copy_from_slice(&bytes[..copy_len]);
                name_arr
            },
        };

        let dest_ptr = packs_buf_ptr + i as u64 * core::mem::size_of::<PackInfo>() as u64;
        let res = safe_memcpy_to_user(dest_ptr, (&raw const pack_info) as u64, core::mem::size_of::<PackInfo>());
        if !res {
            proc.set_syscall_return(&[u64::MAX]);
            return;
        }
    }

    let total_packs = all_packs.len() as u64;
    let written_packs = total_packs.min(packs_buf_size);
    proc.set_syscall_return(&[written_packs, total_packs]);
}

fn map_pack(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
    let namespace_id = args.get_namespace_id();
    let pack_name_len = args.get_arg(0);
    let pack_name_ptr = args.get_arg(1);
    let base_index = args.get_arg(2);

    if base_index + 32 > u32::MAX as u64 {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }

    let Some(pack_name) = syscall::string_from_args(pack_name_ptr, pack_name_len) else {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    };

    println!("Mapping syscall pack '{pack_name}' at base index {base_index} in namespace {namespace_id}");

    let (pack, pack_id) = match syscall_registry::get_syscall_pack(&pack_name) {
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
    drop(proc_mutable);

    if syscall_namespace.map_syscall_pack(base_index as u32, pack, pack_id).is_err() {
        proc.set_syscall_return(&[u64::MAX]);
        return;
    }

    proc.set_syscall_return(&[0]);
}

fn unmap_pack(args: &SyscallCpuState, proc: &Arc<ProcessData>) {
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
