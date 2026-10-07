mod ap_startup;
pub mod cpu_init;

use std::println;

use crate::{
    acpi::{
        Processor,
        cpu_locals::{CpuLocals, add_cpu_locals},
        smp::{send_cpu_locals, send_u64, wait_for_cpus},
    },
    arch::{
        self,
        cpu_locals::ArchCpuLocals,
        interrupts::{
            self, LAPIC_REGISTERS,
            idt::{IDT_POINTER, TablePointer},
        },
        multiprocessing::ap_startup::ap_started_wait_loop,
    },
    memory::{
        addresses::{PhysAddr, VirtAddr, translate_virt_phys_addr},
        stack::{KERNEL_STACK_SIZE_PAGES, prepare_kernel_stack},
    },
    msr::{get_msr, get_mtrr_cap, get_mtrr_def_type},
};

pub fn init_single_cpu(trampoline_destination: *mut u8, cpu: (usize, &Processor)) {
    let start_frame = (trampoline_destination as usize) >> 12;
    let comm_lock = unsafe { trampoline_destination.add(56) };

    let ap_stack_top = prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES);

    unsafe { (trampoline_destination.add(32) as *mut u64).write_volatile(ap_stack_top.0) };

    let lapic_registers = unsafe { LAPIC_REGISTERS.assume_init_mut() };
    println!("Waking up CPU {}", cpu.1.apic_id);
    if cpu.1.apic_id == 255 {
        panic!("invalid cpu: {:?}", cpu);
    }
    let ap_gdt = interrupts::create_new_gdt(ap_stack_top);

    lapic_registers.send_init_ipi(cpu.1.apic_id);
    std::thread::sleep(std::time::Duration::from_millis(10));

    lapic_registers.send_startup_ipi(cpu.1.apic_id, start_frame as u8);
    std::thread::sleep(std::time::Duration::from_millis(100));

    lapic_registers.send_startup_ipi(cpu.1.apic_id, start_frame as u8);

    send_mtrrs(comm_lock);
    send_cr_registers(comm_lock);

    let ap_local = CpuLocals::new(
        ap_stack_top,
        KERNEL_STACK_SIZE_PAGES as u64,
        cpu.1.apic_id,
        cpu.1.processor_id,
        ArchCpuLocals { gdt_ptr: ap_gdt },
    );
    let ap_local_ptr = add_cpu_locals(ap_local);

    send_cpu_locals(ap_local_ptr.0, comm_lock);
    wait_for_cpus(cpu.0 as u8 + 1);
}

pub fn set_trampoline_values(trampoline_destination: *mut u8, trampoline_phys: PhysAddr) {
    let cr3: u64;

    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) cr3,
        );
        assert!(cr3 < 2_u64.pow(32));
        let gdt_ptr = interrupts::STATIC_GDT_PTR;
        let gdt_ptr = TablePointer {
            limit: gdt_ptr.limit,
            base: translate_virt_phys_addr(VirtAddr(gdt_ptr.base), Some(arch::memory::current_root()))
                .expect("page of a static should be mapped")
                .0,
        };
        let wait_loop_ptr = ap_started_wait_loop as *const () as u64;

        //copy physical start into mov instruction
        (trampoline_destination.add(4) as *mut u32).write_volatile(trampoline_phys.0 as u32);

        (trampoline_destination.add(14) as *mut TablePointer).write_volatile(gdt_ptr);
        (trampoline_destination.add(24) as *mut u64).write_volatile(cr3);
        (trampoline_destination.add(40) as *mut u64).write_volatile(wait_loop_ptr);
        (trampoline_destination.add(48) as *mut u64).write_volatile(get_mtrr_def_type());
    }
}

fn send_mtrrs(comm_lock: *mut u8) {
    send_u64(get_mtrr_def_type(), comm_lock);
    send_u64(get_msr(0x250), comm_lock);
    send_u64(get_msr(0x258), comm_lock);
    send_u64(get_msr(0x259), comm_lock);
    send_u64(get_msr(0x268), comm_lock);
    send_u64(get_msr(0x269), comm_lock);
    send_u64(get_msr(0x26A), comm_lock);
    send_u64(get_msr(0x26B), comm_lock);
    send_u64(get_msr(0x26C), comm_lock);
    send_u64(get_msr(0x26D), comm_lock);
    send_u64(get_msr(0x26E), comm_lock);
    send_u64(get_msr(0x26F), comm_lock);

    let n = get_mtrr_cap() & 0xFF;
    for i in 0..n {
        send_u64(get_msr(0x200 + (i as u32 * 2)), comm_lock);
        send_u64(get_msr(0x201 + (i as u32 * 2)), comm_lock);
    }

    send_u64(get_msr(0xC0000080), comm_lock);
}

fn send_cr_registers(comm_lock: *mut u8) {
    unsafe {
        let cr0: u64;
        let cr3: u64;
        let cr4: u64;

        core::arch::asm!(
            "mov {cr0}, cr0",
            "mov {cr3}, cr3",
            "mov {cr4}, cr4",

            cr0 = out(reg) cr0,
            cr3 = out(reg) cr3,
            cr4 = out(reg) cr4
        );
        send_u64(cr0, comm_lock);
        send_u64(cr3, comm_lock);
        send_u64(cr4, comm_lock);
    }
}

fn send_idt(comm_lock: *mut u8) {
    let idt_ptr_addr = core::ptr::addr_of!(IDT_POINTER);
    send_u64(idt_ptr_addr as u64, comm_lock);
}
