use crate::memory::addresses::*;

use bitfield::bitfield;

#[repr(C, packed)]
pub struct HpetTable {
    header: crate::acpi::sdt::AcpiSdtHeader,
    et_block_id: EventTimerBlockID,
    base_addr: AcpiMemoryDescriptor,
    hpet_number: u8,
    min_count: u16,
    page_prot_attr: u8,
}

impl HpetTable {
    pub fn get_addr(&self) -> PhysAddr {
        PhysAddr(self.base_addr.addr)
    }
}

bitfield! {
    struct EventTimerBlockID(u32);
    impl Debug;
    pci_vendor_id, _: 31, 16;
    legacy_replacement_capable, _: 15;
    count_size_cap, _: 13;
    num_comparators, _: 12, 8;
    hardware_rev_id, _: 7, 0;
}

#[repr(C, packed)]
struct AcpiMemoryDescriptor {
    mem_space: AcpiMemorySpace,
    reg_bit_width: u8,
    reg_bit_offset: u8,
    rreserved: u8,
    addr: u64,
}

#[repr(u8)]
enum AcpiMemorySpace {
    SystemMemory = 0,
    SystemIO = 1,
}
