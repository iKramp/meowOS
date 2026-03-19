use crate::interrupts::InterruptProcessorState;
use crate::proc::PROCESS_ID_COUNTER;
use crate::proc::ProcessData;
use crate::proc::SCHEDULER;
use crate::proc::process_data::CpuStateType;
use std::error::ErrorCode;
use std::lock_w_info;
use std::string::ToString;
use std::sync::arc::Arc;
use std::{
    mem_utils::{self, VirtAddr, memset_physical_addr},
    println,
};

use crate::{
    memory::{self, paging::PageTree},
    proc::{MappedMemoryRegion, MemoryContext, Pid},
};

use super::info::ContextInfo;

const DEFAULT_PROC_STACK_SIZE: usize = 0x4000; // 8KB

pub fn create_process(context_info: &ContextInfo) -> Result<Pid, ErrorCode> {
    println!("creating process with context: {:#?}", context_info);
    let pid = Pid(PROCESS_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed));
    let is_32_bit = context_info.is_32_bit();
    let cmdline = context_info.cmdline().to_string().into_boxed_str();
    let rip = context_info.entry_point().0;
    let memory_context = build_mem_context_for_new_proc(context_info)?;
    let stack = memory_context
        .owned_memory_regions
        .iter()
        .find(|region| (*region.name).eq("[stack]"))
        .expect("stack must exist for each proc");
    let rsp = stack.base.0 + (stack.size_pages as u64 * 0x1000) - 16; //-16 just in case (ret val and other things are 0)

    let cpu_state = InterruptProcessorState::new(rip, rsp);
    let process_data = ProcessData::new(
        pid,
        is_32_bit,
        cmdline,
        Arc::new(memory_context),
        CpuStateType::Interrupt(cpu_state),
    );

    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.accept_new_process(pid, process_data);
    Ok(pid)
}

pub fn build_generic_memory_context(context: &ContextInfo) -> MemoryContext {
    let mut memory_tree = build_generic_memory_tree();

    // map memory regions
    for region in context.mem_regions().iter() {
        let start = region.start().0;
        debug_assert!(start % 0x1000 == 0, "region start not page aligned");
        let end = start + region.size_pages() as u64 * 0x1000;
        for page_addr in (start..end).step_by(0x1000) {
            let _phys_addr_map = memory_tree.allocate_set_virtual(None, VirtAddr(page_addr));
            let page = memory_tree
                .get_page_table_entry_mut(VirtAddr(page_addr))
                .expect("page must exist after allocation");
            page.set_writeable(region.flags().is_writeable());
            page.set_user_accessible(true);

            if region.flags().is_executable() {
                memory_tree.set_execute(VirtAddr(page_addr));
            }
        }
    }

    for mem_init in context.mem_init() {
        let first_page = mem_init.0.0 & (!0xfff);
        let last_page = (mem_init.0.0 + mem_init.1.len() as u64) & (!0xfff); //inclusive
        for page_addr in (first_page..=last_page).step_by(0x1000) {
            let page = memory_tree
                .get_page_table_entry_mut(VirtAddr(page_addr))
                .expect("page must exist after allocation");
            let start_mem_addr = page_addr.max(mem_init.0.0);
            let start_data_index = (start_mem_addr - mem_init.0.0) as usize;
            let mem_offset = start_mem_addr & 0xFFF;
            let end_data_index = mem_init.1.len().min(start_data_index + 0x1000 - mem_offset as usize);

            let physical_addr = page.address();

            unsafe {
                mem_utils::memcopy_physical_buffer(physical_addr + mem_offset, &mem_init.1[start_data_index..end_data_index])
            }
        }
    }

    MemoryContext {
        initialized: true,
        is_32_bit: context.is_32_bit(),
        page_tree: memory_tree,
        owned_memory_regions: context
            .mem_regions()
            .iter()
            .map(|region| MappedMemoryRegion {
                name: context.path().to_string().into_boxed_str(),
                base: VirtAddr(region.start().0),
                size_pages: region.size_pages() as u64,
            })
            .collect(),
    }
}

pub fn build_mem_context_for_new_proc(context: &ContextInfo) -> Result<MemoryContext, ErrorCode> {
    let mut generic_context = build_generic_memory_context(context);
    let stack_size_pages = DEFAULT_PROC_STACK_SIZE.div_ceil(0x1000) as u8; // convert to pages

    //add stack
    if let Err(e) = add_stack(&mut generic_context, stack_size_pages) {
        tear_down_mem_context(&generic_context);
        return Err(e);
    }
    Ok(generic_context)
}

pub fn add_stack(context: &mut MemoryContext, stack_size_pages: u8) -> Result<(), ErrorCode> {
    let highest_userspace_addr: u64 = if context.is_32_bit {
        //highest address is 0xFFFF_FFFF, highest quarter is kernel on 32 bit applications
        //The kernel here will STILL be in higher half of 64(48) bit address space, but maybe
        //applications assume their address can't be in the highest qurter
        0xC000_0000
    } else {
        //48 bits for addressing, so highest userspace addr is 0x7FFF_FFFF_FFFF
        0x8000_0000_0000
    };

    let stack_reserve_pages = stack_size_pages as u64 + 1;
    let mem_tree = &mut context.page_tree;
    let stack_search_page = (highest_userspace_addr >> 12) - 1;

    let mut top_page = 0;
    'top_loop: for _top_page in (0..stack_search_page).rev() {
        for page in (_top_page - stack_reserve_pages + 1)..=_top_page {
            if mem_tree.get_page_table_entry_mut(VirtAddr(page << 12)).is_some() {
                //found a page that is already mapped, so we can't use this address
                continue 'top_loop;
            }
        }
        top_page = _top_page;
        break;
    }

    //found a free stack at this address
    for page in (top_page - stack_reserve_pages + 2)..=top_page {
        match mem_tree.allocate_set_virtual(None, VirtAddr(page << 12)) {
            Ok(_) => {}
            Err(e) => {
                for page in (top_page - stack_reserve_pages + 2)..page {
                    mem_tree.deallocate(VirtAddr(page << 12));
                }
                return Err(e);
            }
        }
        let entry = mem_tree
            .get_page_table_entry_mut(VirtAddr(page << 12))
            .expect("page must exist after allocation");
        entry.set_writeable(true);
        entry.set_no_execute(true);
        entry.set_user_accessible(true);
    }
    //add a non-accessible page to catch stack overflows
    let overflow_page = top_page - stack_reserve_pages + 1;
    println!("allocating stack overflow page at {:#X}", overflow_page << 12);
    match mem_tree.allocate_set_virtual(None, VirtAddr(overflow_page << 12)) {
        Ok(_) => {}
        Err(e) => {
            for page in (top_page - stack_reserve_pages + 2)..=top_page {
                mem_tree.deallocate(VirtAddr(page << 12));
            }
            return Err(e);
        }
    }
    let entry = mem_tree
        .get_page_table_entry_mut(VirtAddr(overflow_page << 12))
        .expect("page must exist after allocation");
    entry.set_writeable(true);
    entry.set_no_execute(true);
    entry.set_user_accessible(false);

    let stack = MappedMemoryRegion {
        name: "[stack]".to_string().into_boxed_str(),
        base: VirtAddr(((top_page - stack_size_pages as u64) << 12) + 0x1000),
        size_pages: stack_size_pages as u64,
    };
    context.owned_memory_regions.push(stack);

    Ok(())
}

pub fn build_generic_memory_tree() -> PageTree {
    let page_tree_root = memory::physical_allocator::allocate_frame();
    unsafe { memset_physical_addr(page_tree_root, 0x0, 0x1000) };
    let mut new_page_tree = PageTree::new(page_tree_root);

    let existing_page_tree = PageTree::new(PageTree::get_level4_addr());
    existing_page_tree.copy_higher_half(&mut new_page_tree);

    new_page_tree
}

pub fn tear_down_mem_context(_context: &MemoryContext) {
    todo!("tear down mem context")
}
