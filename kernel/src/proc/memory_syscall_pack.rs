use std::{boxed::Box, mem_utils::VirtAddr, println, sync::arc::Arc};

use crate::{
    memory::{self, VirtualMemoryRangeManagementMode},
    proc::{
        MemoryNamespace, MemoryRangeType, ProcessData,
        syscall::{self, SyscallCpuState},
    },
};

pub fn init_mem_syscalls() {
    let handlers = [make_region, remove_region, list_regions, set_prot, mmap, munmap];

    let mem_syscalls = crate::proc::syscall::SyscallPack::new(Box::new(handlers));
    crate::proc::syscall::register_syscall_pack("memory".into(), Arc::new(mem_syscalls));
}

fn make_region(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();

    let start_addr = args.get_arg(0);
    let size_order = args.get_arg(1);
    let permissions = args.get_arg(2) as u8;
    let region_type = args.get_arg(3);
    let management_mode = args.get_arg(4);
    let region_name_len = args.get_arg(5);
    let region_name_ptr = args.get_arg(6);

    let Some(region_name) = syscall::string_from_args(region_name_ptr, region_name_len) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let Some(range_capacity) = memory::VirtualMemoryRangeCapacity::from_level(size_order as u8) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };
    let start_addr = range_capacity.align_down(VirtAddr(start_addr));
    let permissions = memory::VirtualMemoryRangePermissions(permissions);
    let Some(region_type) = MemoryRangeType::from_u32(region_type as u32) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let Some(management_mode) = VirtualMemoryRangeManagementMode::from_u64(management_mode) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    //------checks passed------

    let mutable = proc.get_mutable();
    let namespaces = mutable.get_namespaces();
    let Some(memory_namespace) = namespaces.get_namespace::<MemoryNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let virtual_memory_range = memory::VirtualMemoryRange::create(range_capacity, permissions, management_mode);

    let res = memory_namespace.add_mem_range(
        Arc::new(virtual_memory_range),
        region_name.into_boxed_str(),
        region_type,
        start_addr,
    );
    let err_code = if res.is_ok() { 0 } else { u64::MAX };
    proc.set_syscall_return(&[err_code]);

    false
}

fn remove_region(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let region_id = args.get_arg(0) as u32;

    let mutable = proc.get_mutable();
    let namespaces = mutable.get_namespaces();
    let Some(memory_namespace) = namespaces.get_namespace::<MemoryNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };
    let res = memory_namespace.remove_mem_range_by_id(region_id);
    let err_code = if res.is_ok() { 0 } else { u64::MAX };
    proc.set_syscall_return(&[err_code]);
    false
}

fn list_regions(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    println!("list regions called with args: {:#x?}, proc: {:#x?}", args, proc);
    todo!()
}

fn set_prot(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    println!("set prot called with args: {:#x?}, proc: {:#x?}", args, proc);
    todo!()
}

fn mmap(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let region_id = args.get_arg(0) as u32;
    let mut offset = args.get_arg(1);
    let num_pages = args.get_arg(2);

    let mutable = proc.get_mutable();
    let namespaces = mutable.get_namespaces();
    let Some(memory_namespace) = namespaces.get_namespace::<MemoryNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let ret = if region_id == 0 {
        memory_namespace.get_range_from_address(VirtAddr(offset))
    } else {
        memory_namespace.get_range_from_id(region_id)
    };

    let Some((range, base_addr)) = ret else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };
    if region_id == 0 {
        offset -= base_addr.0;
    }

    let pages_start = offset as u32 / 0x1000;
    let pages_end = pages_start + num_pages as u32;

    let ret = range.allocate_manual_external(pages_start..pages_end);
    let err_code = if ret.is_ok() { 0 } else { u64::MAX };
    proc.set_syscall_return(&[err_code]);

    false
}

fn munmap(args: &SyscallCpuState, proc: &Arc<ProcessData>) -> bool {
    let namespace_id = args.get_namespace_id();
    let region_id = args.get_arg(0) as u32;
    let mut offset = args.get_arg(1);
    let num_pages = args.get_arg(2);

    let mutable = proc.get_mutable();
    let namespaces = mutable.get_namespaces();
    let Some(memory_namespace) = namespaces.get_namespace::<MemoryNamespace>(namespace_id) else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };

    let ret = if region_id == 0 {
        memory_namespace.get_range_from_address(VirtAddr(offset))
    } else {
        memory_namespace.get_range_from_id(region_id)
    };

    let Some((range, base_addr)) = ret else {
        proc.set_syscall_return(&[u64::MAX]);
        return false;
    };
    if region_id == 0 {
        offset -= base_addr.0;
    }

    let pages_start = offset as u32 / 0x1000;
    let pages_end = pages_start + num_pages as u32;

    let ret = range.free_manual_external(pages_start..pages_end);
    let err_code = if ret.is_ok() { 0 } else { u64::MAX };
    proc.set_syscall_return(&[err_code]);

    false
}
