mod aml;
mod apic;
mod fadt;
mod hpet;
mod ioapic;
mod lapic_timer;
mod madt;
mod platform_info;
mod rsdp;
mod rsdt;
mod sdt;
mod smp;

use std::{
    collections::btree_map::BTreeMap,
    mem_utils::{PhysAddr, get_at_physical_addr},
};

pub use apic::LAPIC_REGISTERS;
use fadt::Fadt;
pub use hpet::HpetTable;
use madt::Madt;
use platform_info::PlatformInfo;
pub use smp::cpu_locals;

use crate::{limine::LIMINE_BOOTLOADER_REQUESTS, memory::PAGE_TREE_ALLOCATOR, println, printlnc};

static mut PLATFORM_INFO: Option<PlatformInfo> = None;
pub static mut ACPI_TABLE_MAP: BTreeMap<&str, PhysAddr> = BTreeMap::new();

//this is safe because it's set when only 1 core is active, after that it's read only
pub fn get_table<T: 'static>(name: &str) -> Option<&T> {
    let addr = unsafe { ACPI_TABLE_MAP.get(name).copied()? };
    unsafe { Some(get_at_physical_addr::<T>(addr)) }
}

pub fn get_platform_info() -> &'static PlatformInfo {
    unsafe {
        let Some(platform_info) = &PLATFORM_INFO else {
            panic!("platform info not initialized");
        };
        platform_info
    }
}

pub fn read_tables() {
    let rsdp = rsdp::get_rsdp_table(unsafe { (*LIMINE_BOOTLOADER_REQUESTS.rsdp_request.info).rsdp as u64 })
        .expect("This os doesn not support PCs without ACPI");
    let rsdt = rsdt::get_rsdt(&rsdp);
    assert!(rsdt.validate());
    println!("rsdt is valid");

    let tables = rsdt.get_tables();
    for table in &tables {
        unsafe {
            let header = std::mem_utils::get_at_physical_addr::<sdt::AcpiSdtHeader>(*table);
            let signature = std::str::from_utf8(&header.signature).expect("signatures are ascii, error in mem read");
            println!("Found ACPI table: {} at physical address {}", signature, table.0);
            ACPI_TABLE_MAP.insert(signature, *table);
        }
    }
    println!("Acpi tables read");
}

pub fn init_acpi() {
    let fadt = get_table::<Fadt>("FACP").expect("fadt should be present");
    let madt = get_table::<Madt>("APIC").expect("madt should be present");

    let entries = madt.get_madt_entries();
    let platform_info = platform_info::PlatformInfo::new(&entries, std::mem_utils::PhysAddr(madt.local_apic_address as u64));
    //override madt apic address if it exists in entries
    println!("initing APIC");
    unsafe {
        PLATFORM_INFO = Some(platform_info);
    };
    let platform_info = get_platform_info();
    cpu_locals::init(platform_info);

    apic::enable_apic(platform_info, platform_info.boot_processor.processor_id);
    ioapic::init_ioapic(platform_info);

    smp::wake_cpus(platform_info);
    printlnc!((0, 255, 0), "ACPI initialized and APs started");
    unsafe { PAGE_TREE_ALLOCATOR.unmap_lower_half() };

    //after loading dsdt
    /*
        for table in &rsdt.get_tables() {
            unsafe {
                let header = std::mem_utils::get_at_physical_addr::<sdt::AcpiSdtHeader>(*table);
                if &header.signature == b"SSDT" {
                    //parse secondary tables
                    //actually don't this shit is difficult af
                }
            }
        }
    */

    let _dsdt_addr = std::mem_utils::translate_phys_virt_addr(PhysAddr(fadt.dsdt as u64));
    //let _aml_code = aml::AmlCode::new(dsdt_addr.0 as *const u8);
}

pub fn init_acpi_ap(processor_id: u8) {
    unsafe {
        let Some(platform_info) = &PLATFORM_INFO else {
            panic!("should be impossible, acpi tables are not loaded but APs were initialized");
        };
        apic::enable_apic(platform_info, processor_id);
    }
}
