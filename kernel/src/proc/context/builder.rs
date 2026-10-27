use crate::arch;
use crate::interrupts::InterruptProcessorState;
use crate::memory::VirtualMemoryRange;
use crate::memory::VirtualMemoryRangeGrowDirection;
use crate::memory::VirtualMemoryRangeManagementMode;
use crate::memory::VirtualMemoryRangePermissions;
use crate::memory::addresses::VirtAddr;
use crate::proc::MemoryRangeType;
use crate::proc::PROCESS_ID_COUNTER;
use crate::proc::ProcNamespace;
use crate::proc::ProcNamespaces;
use crate::proc::ProcessData;
use crate::proc::SCHEDULER;
use crate::proc::SyscallNamespace;
use crate::proc::namespaces::FilesystemNamespace;
use crate::proc::namespaces::MemoryNamespace;
use crate::proc::process_data::CpuStateType;
use crate::vfs::file::FileHandle;
use std::alloc::boxed::Box;
use std::error::KernelError;
use std::kerror;
use std::lock_w_info;
use std::println;
use std::string::ToString;
use std::sync::arc::Arc;

use crate::{memory, proc::Pid};

use super::info::ContextInfo;

const DEFAULT_PROC_STACK_SIZE: usize = 0x1000; // 1kB initial stack

pub fn create_process_from_context(context_info: &ContextInfo, cwd: FileHandle) -> Result<Pid, KernelError> {
    println!("creating process with context: {:#?}", context_info);
    let pid = Pid(PROCESS_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed));
    let is_32_bit = context_info.is_32_bit();
    let cmdline = context_info.cmdline().to_string().into_boxed_str();
    let rip = context_info.entry_point().0;

    let (memory_namespace, argc_argv) = build_mem_namespace_for_new_proc(context_info)?;
    let dynamic_namespace_data = lock_w_info!(memory_namespace.dynamic_data);

    let stack = dynamic_namespace_data
        .regions()
        .iter()
        .find(|region| region.range_type == MemoryRangeType::Stack)
        .expect("stack must exist for each proc");
    let rsp = stack.shared_range.reserved_range(stack.map_address).end.0 - 16;
    drop(dynamic_namespace_data);

    let memory_namespace = Arc::new(memory_namespace);
    let syscall_namespace = Arc::new(SyscallNamespace::default(crate::proc::get_namespace_id()));
    let filesystem_namespace = Arc::new(FilesystemNamespace::new(crate::proc::get_namespace_id(), cwd));

    let mut cpu_state = InterruptProcessorState::new(rip, rsp);
    cpu_state.set_arg_registers(argc_argv.0, argc_argv.1, 0);
    let process_data = ProcessData::new(
        pid,
        is_32_bit,
        cmdline,
        CpuStateType::Interrupt(cpu_state),
        ProcNamespaces::new(memory_namespace, syscall_namespace, filesystem_namespace),
    );

    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.accept_new_process(pid, process_data);

    Ok(pid)
}
pub fn build_initialized_memory_namespace(
    context: &ContextInfo,
    mem_namespace: MemoryNamespace,
) -> Result<MemoryNamespace, KernelError> {
    let mut perms = VirtualMemoryRangePermissions(0); //allow writing
    perms.set_write(true);

    let proc_mem_range = VirtualMemoryRange::create(
        memory::VirtualMemoryRangeCapacity::_05TB,
        perms,
        VirtualMemoryRangeManagementMode::Manual,
    );
    let proc_mem_range_bounds = proc_mem_range.reserved_range(VirtAddr(0));

    // map memory regions
    for region in context.mem_regions().iter() {
        let start = region.start();
        debug_assert!(start.0 % 0x1000 == 0, "region start not page aligned");
        let end = start + region.size_pages() as u64 * 0x1000;
        if end > proc_mem_range_bounds.end {
            println!(
                "region end {:#x?} exceeds process memory range bounds {:#x?}",
                end, proc_mem_range_bounds.end.0
            );
            return kerror!(InvalidProcessFile);
        }

        let page_start = start.0 / 0x1000;
        let page_end = end.0 / 0x1000;

        proc_mem_range.allocate_manual_external(page_start as u32..page_end as u32)?;
    }

    let mem_range = Arc::new(proc_mem_range);

    mem_namespace
        .add_mem_range(
            mem_range.clone(),
            context.path().to_string().into_boxed_str(),
            MemoryRangeType::Code,
            VirtAddr(0),
        )
        .expect("mapping process memory failed");

    for mem_init in context.mem_init() {
        let dest_ptr = mem_init.0.0 as *mut u8;
        let src_ptr = mem_init.1.as_ptr();
        let len = mem_init.1.len();
        unsafe {
            core::ptr::copy_nonoverlapping(src_ptr, dest_ptr, len);
        }
    }

    //TODO: this fucks up permissions per range, fix

    //set prot
    for region in context.mem_regions().iter() {
        let start = region.start();
        let end = start + region.size_pages() as u64 * 0x1000;

        let mut perms = VirtualMemoryRangePermissions(0);
        perms.set_write(region.flags().is_writeable());
        perms.set_execute(region.flags().is_executable());

        memory::set_prot(arch::memory::current_root(), start..end, perms, 4, VirtAddr(0));
    }

    Ok(mem_namespace)
}

//returns (memory_namespace, (argc, argv))
pub fn build_mem_namespace_for_new_proc(context: &ContextInfo) -> Result<(MemoryNamespace, (u64, u64)), KernelError> {
    let mem_namespace = build_empty_memory_namespace();

    let current_root = arch::memory::current_root();
    arch::memory::set_page_tree_root(mem_namespace.page_tree_root());

    let mut mem_namespace = build_initialized_memory_namespace(context, mem_namespace)?;
    let stack_size_pages = DEFAULT_PROC_STACK_SIZE.div_ceil(0x1000) as u8; // convert to pages

    //add stack
    if let Err(e) = add_stack(&mut mem_namespace, stack_size_pages) {
        arch::memory::set_page_tree_root(current_root);
        return Err(e);
    }

    let argc_argv = match add_args(&context.args, &mut mem_namespace) {
        Ok((argc, argv)) => (argc, argv),
        Err(e) => {
            arch::memory::set_page_tree_root(current_root);
            return Err(e);
        }
    };

    arch::memory::set_page_tree_root(current_root);

    Ok((mem_namespace, argc_argv))
}

pub fn add_stack(mem_namespace: &mut MemoryNamespace, stack_size_pages: u8) -> Result<(), KernelError> {
    let Some(free_area) = mem_namespace.find_hole(memory::VirtualMemoryRangeCapacity::_1GB) else {
        println!("no free area for stack");
        return kerror!(OutOfMemory);
    };

    let mut permissions = VirtualMemoryRangePermissions(0);
    permissions.set_write(true);
    permissions.set_execute(false);
    let mem_range = VirtualMemoryRange::create(
        memory::VirtualMemoryRangeCapacity::_1GB,
        permissions,
        VirtualMemoryRangeManagementMode::Managed(VirtualMemoryRangeGrowDirection::Down),
    );
    mem_range.expand_to(stack_size_pages as u32).expect("adding stack failed");
    let stack_highest_addr = mem_range.reserved_range(free_area).end;

    mem_namespace.add_mem_range(
        Arc::new(mem_range),
        "[stack]".to_string().into_boxed_str(),
        crate::proc::MemoryRangeType::Stack,
        free_area,
    )?;

    unsafe { *((stack_highest_addr.0 - 0x08) as *mut u64) = 0 };
    unsafe { *((stack_highest_addr.0 - 0x10) as *mut u64) = 0 };
    println!("added stack mem range");
    Ok(())
}

pub fn add_args(args: &[Box<str>], mem_namespace: &mut MemoryNamespace) -> Result<(u64, u64), KernelError> {
    let args_count = args.len() as u64;
    let args_size = args.iter().map(|arg| arg.len() + 1).sum::<usize>() as u64; // +1 for null terminator +8 for table pointer
    let alignment_padding = (8 - (args_size % 8)) % 8; // padding to align to 8 bytes
    let args_table_size = args_count * 8; // 8 bytes per pointer
    let combined_size = args_size + args_table_size + alignment_padding;

    if combined_size == 0 {
        return Ok((0, 0));
    }

    let capacity = if combined_size >= (0x1000 << (9 * 2)) {
        memory::VirtualMemoryRangeCapacity::_1GB
    } else if combined_size >= (0x1000 << 9) {
        memory::VirtualMemoryRangeCapacity::_2MB
    } else {
        memory::VirtualMemoryRangeCapacity::_4KB
    };

    let Some(free_area) = mem_namespace.find_hole(capacity) else {
        println!("no free area for args");
        return kerror!(OutOfMemory);
    };

    let mut permissions = VirtualMemoryRangePermissions(0);
    permissions.set_write(true);
    permissions.set_execute(false);
    let mem_range = VirtualMemoryRange::create(capacity, permissions, VirtualMemoryRangeManagementMode::Manual);

    let start_page = 0;
    let end_page = combined_size.div_ceil(0x1000) as u32;

    mem_range.allocate_manual_external(start_page..end_page)?;

    let args_start_addr = free_area.0;
    let args_end_addr = args_start_addr + args_size;

    mem_namespace.add_mem_range(
        Arc::new(mem_range),
        "[args]".to_string().into_boxed_str(),
        crate::proc::MemoryRangeType::StaticData,
        free_area,
    )?;

    println!(
        "added args mem range: start: {:#x?}, end: {:#x?}, size: {:#x?}",
        args_start_addr, args_end_addr, combined_size
    );

    // copy args to the allocated memory
    let mut current_addr = args_start_addr as usize;
    let table_start_addr = args_end_addr + alignment_padding;
    let mut current_table_addr = table_start_addr as usize;

    println!("table start pointer: {:#x?}", current_table_addr);

    for arg in args.iter() {
        println!("copying arg: {:?} to addr: {:#x?}", arg, current_addr);
        println!("table entry for arg at: {:#x?}", current_table_addr);
        unsafe {
            *((current_table_addr) as *mut u64) = current_addr as u64;
            current_table_addr += 8;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(arg.as_ptr(), current_addr as *mut u8, arg.len());
            *((current_addr + arg.len()) as *mut u8) = 0; // null terminator
            current_addr += arg.len() + 1;
        }
    }

    Ok((args_count, table_start_addr))
}

pub fn build_empty_memory_namespace() -> MemoryNamespace {
    let namespace = MemoryNamespace::create_empty(crate::proc::get_namespace_id()).expect("failed to create memory namespace");

    let new_page_tree_root = namespace.page_tree_root();
    let existing_page_tree_root = arch::memory::current_root();
    memory::copy_higher_half(existing_page_tree_root, new_page_tree_root);

    namespace
}
