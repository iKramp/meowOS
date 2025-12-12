use crate::{
    acpi::cpu_locals::add_cpu_locals,
    interrupts::{
        self,
        idt::{IDT_POINTER, TablePointer},
    },
    memory::{
        PAGE_TREE_ALLOCATOR,
        paging::{LiminePat, PageTree},
        stack::{KERNEL_STACK_SIZE_PAGES, prepare_kernel_stack},
    },
    msr::{get_msr, get_mtrr_cap, get_mtrr_def_type},
    println,
};
use core::sync::atomic::{AtomicBool, AtomicU8};
use std::mem_utils::VirtAddr;

pub static mut CPU_LOCK: AtomicBool = AtomicBool::new(false);
pub static mut CPUS_INITIALIZED: AtomicU8 = AtomicU8::new(0);

//custom data starts at 0x4 from ap_startup

use crate::acpi::{LAPIC_REGISTERS, platform_info::PlatformInfo};

pub fn wake_cpus(platform_info: &PlatformInfo) {
    copy_trampoline();

    let start_page = unsafe { crate::memory::TRAMPOLINE_RESERVED.0 } >> 12;
    unsafe {
        //change bsp's gdt, the static one will be used by each AP before switching to their own
        //GDTs
        //i could probably reuse current stack, as kernel doesn't preserve stack across task
        //switches

        let destination = crate::memory::TRAMPOLINE_RESERVED.0 as *mut u8;
        let comm_lock = destination.add(56);
        for cpu in platform_info.application_processors.iter().enumerate() {
            let ap_stack_top = prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES);
            PAGE_TREE_ALLOCATOR.print_entries(VirtAddr(0xfffffffffff92fe8));
            (destination.add(32) as *mut u64).write_volatile(ap_stack_top.0);

            let lapic_registers = LAPIC_REGISTERS.assume_init_mut();
            println!("Waking up CPU {}", cpu.1.apic_id);
            if cpu.1.apic_id == 255 {
                panic!("invalid cpu: {:?}", cpu);
            }
            let ap_gdt = interrupts::create_new_gdt(ap_stack_top);

            lapic_registers.send_init_ipi(cpu.1.apic_id);
            std::thread::sleep(std::time::Duration::from_millis(10));

            lapic_registers.send_startup_ipi(cpu.1.apic_id, start_page as u8);
            std::thread::sleep(std::time::Duration::from_millis(100));

            lapic_registers.send_startup_ipi(cpu.1.apic_id, start_page as u8);

            send_mtrrs(comm_lock);
            send_cr_registers(comm_lock);

            let ap_local = super::cpu_locals::CpuLocals::new(
                ap_stack_top,
                KERNEL_STACK_SIZE_PAGES as u64,
                cpu.1.apic_id,
                cpu.1.processor_id,
                ap_gdt,
            );
            let ap_local_ptr = add_cpu_locals(ap_local);

            send_cpu_locals(ap_local_ptr.0, comm_lock);
            wait_for_cpus(cpu.0 as u8 + 1);
        }
    }
}

fn copy_trampoline() {
    let destination = unsafe { crate::memory::TRAMPOLINE_RESERVED };
    println!("copying trampoline to {:x?}", destination);

    assert!(
        destination.0 <= 0xFFFFF,
        "memory addresss should be less than 1MB to initialize APs"
    );
    let source = super::ap_startup::ap_startup as *const () as *const u8;
    let destination = destination.0 as *mut u8;
    for i in 0..0x1000 {
        unsafe {
            destination.add(i).write_volatile(source.add(i).read_volatile());
        }
    }

    let cr3: u64;

    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            out(reg) cr3,
        );
        assert!(cr3 < 2_u64.pow(32));
        let gdt_ptr = crate::interrupts::STATIC_GDT_PTR;
        let page_tree_root = PageTree::get_level4_addr();
        let gdt_ptr = TablePointer {
            limit: gdt_ptr.limit,
            base: std::mem_utils::translate_virt_phys_addr(VirtAddr(gdt_ptr.base), page_tree_root)
                .expect("page of a static is mapped")
                .0,
        };
        let wait_loop_ptr = super::ap_startup::ap_started_wait_loop as *const () as u64;

        (destination.add(4) as *mut u32).write_volatile(destination as u32);

        (destination.add(14) as *mut TablePointer).write_volatile(gdt_ptr);
        (destination.add(24) as *mut u64).write_volatile(cr3);
        (destination.add(40) as *mut u64).write_volatile(wait_loop_ptr);
        (destination.add(48) as *mut u64).write_volatile(get_mtrr_def_type());
    }
}

pub fn wait_for_cpus(num_cpus: u8) {
    loop {
        let cpus_initialized = unsafe { CPUS_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed) };
        if cpus_initialized == num_cpus {
            break;
        }
    }
}

fn send_cpu_locals(ptr: u64, comm_lock: *mut u8) {
    //TODO:
    send_u64(ptr, comm_lock);
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

fn send_u64(data: u64, comm_lock: *mut u8) {
    send_bytes(&data.to_ne_bytes(), comm_lock);
}

fn send_u16(data: u16, comm_lock: *mut u8) {
    send_bytes(&data.to_ne_bytes(), comm_lock);
}

fn send_bytes(data: &[u8], comm_lock: *mut u8) {
    for byte in data {
        send_byte(*byte, comm_lock);
    }
}

fn send_byte(data_byte: u8, comm_lock: *mut u8) {
    unsafe {
        let mut byte;
        loop {
            byte = 1_u8;
            core::arch::asm!(//obtain comm lock
                "xchg {byte}, [{comm_lock}]",
                byte = inout(reg_byte) byte,
                comm_lock = in(reg) comm_lock,
            );
            if byte != 0 {
                continue;
            }
            let data_ready: u8;
            core::arch::asm!(//check if there's pending data
                "mov {byte}, [{comm_lock}]",
                byte = out(reg_byte) data_ready,
                comm_lock = in(reg) comm_lock.add(1),
            );
            if data_ready == 1 {
                //ap didn't read yet
                core::arch::asm!(//release lock
                    "mov [{comm_lock}], {zero}",
                    "clflush [{comm_lock}]",
                    comm_lock = in(reg) comm_lock,
                    zero = in(reg_byte) 0_u8,
                );
                continue;
            } else {
                break;
            }
        }
        core::arch::asm!(//write data
            "mov [{comm_lock}], {data}",
            "clflush [{comm_lock}]",
            data = in(reg_byte) data_byte,
            comm_lock = in(reg) comm_lock.add(2),
        );
        core::arch::asm!(//set pending data
            "mov [{comm_lock}], {one}",
            "clflush [{comm_lock}]",
            one = in(reg_byte) 1_u8,
            comm_lock = in(reg) comm_lock.add(1),
        );
        core::arch::asm!(//release lock
            "mov [{comm_lock}], {zero}",
            "clflush [{comm_lock}]",
            comm_lock = in(reg) comm_lock,
            zero = in(reg_byte) 0_u8,
        );
    }
}
