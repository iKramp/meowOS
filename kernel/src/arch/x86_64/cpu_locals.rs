use std::println;

use crate::{
    acpi::cpu_locals::CpuLocals,
    arch::interrupts::{self, idt::TablePointer},
    memory::stack::{KERNEL_STACK_SIZE_PAGES, prepare_kernel_stack},
};

pub struct ArchCpuLocals {
    //arch specific cpu locals
    pub gdt_ptr: TablePointer,
}

pub fn init_dummy_cpu_local() -> CpuLocals {
    let bsp_stack_ptr = prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES);
    println!(level:info, "BSP stack ptr: {:016X}, size: {:X}", bsp_stack_ptr.0, KERNEL_STACK_SIZE_PAGES as u64 * 4096);
    let bsp_gdt = interrupts::create_new_gdt(bsp_stack_ptr);
    interrupts::load_gdt(bsp_gdt);
    CpuLocals::new(
        bsp_stack_ptr,
        KERNEL_STACK_SIZE_PAGES as u64,
        0,
        0,
        ArchCpuLocals { gdt_ptr: bsp_gdt },
    )
}
