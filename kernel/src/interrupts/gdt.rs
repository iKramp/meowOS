use crate::memory::addresses::*;
use std::boxed::Box;

use crate::memory::stack::{KERNEL_STACK_SIZE_PAGES, prepare_kernel_stack};

use super::idt::TablePointer;

pub const DOUBLE_FAULT_IST: u16 = 1;
pub const NMI_IST: u16 = 2;
pub const MACHINE_CHECK_IST: u16 = 3;
pub const DEBUG_IST: u16 = 4;

pub const KERNEL_STACK_SIZE_BYTES: usize = KERNEL_STACK_SIZE_PAGES as usize * 0x1000;
pub static mut STATIC_GDT_PTR: TablePointer = TablePointer { limit: 0, base: 0 };

const GDT_LEN: usize = 7;

pub fn load_gdt(ptr: TablePointer) {
    unsafe { core::arch::asm!("lgdt [{}]", in(reg) core::ptr::addr_of!(ptr), options(readonly, nostack, preserves_flags)) };
    set_cs();
    unsafe { core::arch::asm!("mov ax, 0x28", "ltr ax", out("ax") _, options(nostack, preserves_flags, raw)) };
}

#[repr(C, packed)]
#[derive(Debug)]
struct TaskStateSegment {
    padding_1: u32,
    privilege_stack_table: [u64; 3],
    padding_2: u64,
    interrupt_stack_table: [u64; 7],
    padding_3: u64,
    padding_4: u16,
    io_map_base_address: u16,
}

//wrapped to align
#[repr(align(16))]
struct Ist {
    #[allow(unused)] //used
    stack: [u8; KERNEL_STACK_SIZE_BYTES],
}

#[used]
static mut TSS: TaskStateSegment = TaskStateSegment {
    padding_1: 0,
    privilege_stack_table: [0; 3],
    padding_2: 0,
    interrupt_stack_table: [0; 7],
    padding_3: 0,
    padding_4: 0,
    io_map_base_address: core::mem::size_of::<TaskStateSegment>() as u16,
};

fn init_tss(tss: &mut TaskStateSegment, static_stacks: bool, kernel_stack_ptr: Option<u64>) {
    tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize - 1] = {
        #[used]
        static mut STACK: Ist = Ist {
            stack: [0; KERNEL_STACK_SIZE_BYTES],
        };

        if static_stacks {
            core::ptr::addr_of!(STACK) as u64 + KERNEL_STACK_SIZE_BYTES as u64
        } else {
            prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES).0
        }
    };
    tss.interrupt_stack_table[NMI_IST as usize - 1] = {
        #[used]
        static mut STACK: Ist = Ist {
            stack: [0; KERNEL_STACK_SIZE_BYTES],
        };

        if static_stacks {
            core::ptr::addr_of!(STACK) as u64 + KERNEL_STACK_SIZE_BYTES as u64
        } else {
            prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES).0
        }
    };
    tss.interrupt_stack_table[MACHINE_CHECK_IST as usize - 1] = {
        #[used]
        static mut STACK: Ist = Ist {
            stack: [0; KERNEL_STACK_SIZE_BYTES],
        };

        if static_stacks {
            core::ptr::addr_of!(STACK) as u64 + KERNEL_STACK_SIZE_BYTES as u64
        } else {
            prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES).0
        }
    };
    tss.interrupt_stack_table[DEBUG_IST as usize - 1] = {
        #[used]
        static mut STACK: Ist = Ist {
            stack: [0; KERNEL_STACK_SIZE_BYTES],
        };

        if static_stacks {
            core::ptr::addr_of!(STACK) as u64 + KERNEL_STACK_SIZE_BYTES as u64
        } else {
            prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES).0
        }
    };
    tss.privilege_stack_table[0] = {
        #[used]
        static mut STACK: Ist = Ist {
            stack: [0; KERNEL_STACK_SIZE_BYTES],
        };

        if let Some(kernel_stack_ptr) = kernel_stack_ptr {
            kernel_stack_ptr
        } else {
            core::ptr::addr_of!(STACK) as u64 + KERNEL_STACK_SIZE_BYTES as u64
        }
    };
}

#[derive(Debug)]
#[repr(C, align(8))]
pub struct GlobalDescriptorTable {
    table: [SegmentDescriptor; GDT_LEN],
    len: usize,
}

impl GlobalDescriptorTable {
    const fn default() -> Self {
        GlobalDescriptorTable {
            //user segments are inverted because sysret requres user code to be user data + 8
            table: [
                /*0x00*/ create_segment_descriptor(0, 0, 0, 0), //null descriptor
                /*0x08*/ create_segment_descriptor(0, 0xFFFFF, 0x9A, 0xA), //code segment
                /*0x10*/ create_segment_descriptor(0, 0xFFFFF, 0x92, 0xC), //data segment
                /*0x18*/ create_segment_descriptor(0, 0xFFFFF, 0xF2, 0xC), //user data segment
                /*0x20*/ create_segment_descriptor(0, 0xFFFFF, 0xFA, 0xA), //user code segment
                /*0x28*/ create_segment_descriptor(0, 0x0, 0x0, 0x0), //TSS segment placeholder
                /*0x00*/ create_segment_descriptor(0, 0x0, 0x0, 0x0), //TSS segment placeholder
            ],
            len: 5, //len is 2 shorter to account for tss placeholders
        }
    }
}

#[used]
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::default();

impl GlobalDescriptorTable {
    fn pointer(&self) -> TablePointer {
        TablePointer {
            limit: (GDT_LEN * 8 - 1) as u16,
            base: self.table.as_ptr() as u64,
        }
    }

    fn append_128(&mut self, data: (SegmentDescriptor, SegmentDescriptor)) -> usize {
        if self.len >= GDT_LEN - 1 {
            panic!("gdt size exceeded");
        }
        self.table[self.len] = data.0;
        self.len += 1;
        self.table[self.len] = data.1;
        self.len += 1;
        (self.len - 2) << 3
    }
}

#[derive(Debug)]
#[repr(C, packed)]
struct SegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    base_mid: u8,
    access_byte: u8,
    lim_h_flags: u8,
    base_high: u8,
}

const fn create_128_segment_descriptor(
    base: u64,
    limit: u32,
    access_byte: u8,
    flags: u8,
) -> (SegmentDescriptor, SegmentDescriptor) {
    let low = create_segment_descriptor(base, limit, access_byte, flags);
    let high = create_segment_descriptor((base >> 48) & 0xFFFF, ((base >> 32) & 0xFFFF) as u32, 0, 0); //a bit of a hack, we're actually
    //doing a 32 bit base
    (low, high)
}

const fn create_segment_descriptor(base: u64, limit: u32, access_byte: u8, flags: u8) -> SegmentDescriptor {
    SegmentDescriptor {
        limit_low: (limit & 0xFFFF) as u16,
        base_low: (base & 0xFFFF) as u16,
        base_mid: ((base & 0xFF0000) >> 16) as u8,
        access_byte,
        lim_h_flags: ((limit & 0xF0000) >> 16) as u8 | ((flags & 0xF) << 4),
        base_high: ((base & 0xFF000000) >> 24) as u8,
    }
}

pub fn init_boot_gdt() {
    unsafe {
        init_tss(&mut TSS, true, None);
        GDT.append_128(create_128_segment_descriptor(
            core::ptr::addr_of!(TSS) as u64,
            (core::mem::size_of::<TaskStateSegment>() - 1) as u32,
            0x89,
            0x0,
        ));
        STATIC_GDT_PTR = GDT.pointer();
        load_gdt(GDT.pointer());
    };
}

pub fn create_new_gdt(kernel_stack_ptr: VirtAddr) -> TablePointer {
    let mut gdt = Box::new(GlobalDescriptorTable::default());
    let mut tss = Box::new(TaskStateSegment {
        padding_1: 0,
        privilege_stack_table: [0; 3],
        padding_2: 0,
        interrupt_stack_table: [0; 7],
        padding_3: 0,
        padding_4: 0,
        io_map_base_address: core::mem::size_of::<TaskStateSegment>() as u16,
    });
    init_tss(tss.as_mut(), false, Some(kernel_stack_ptr.0));
    gdt.append_128(create_128_segment_descriptor(
        tss.as_ref() as *const _ as u64,
        (core::mem::size_of::<TaskStateSegment>() - 1) as u32,
        0x89,
        0x0,
    ));
    let ptr = gdt.pointer();
    let _ = Box::leak(gdt);
    let _ = Box::leak(tss);
    ptr
}

pub fn set_cs() {
    unsafe {
        core::arch::asm!(
            "push 0x08", //code segment
            "lea rax, [rip + 2f]", //load the address of the label
            "push rax", //push the address of the label
            "retfq", //return far qword

            "2:", //label
            "mov ax, 0x10", //data segment
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            out("rax") _,
            options(nostack, preserves_flags, raw)
        )
    }
}
