mod aml;
mod apic;
mod fadt;
mod hpet;
mod ioapic;
mod lapic_timer;
mod madt;
mod mcfg;
mod platform_info;
mod rsdp;
mod rsdt;
mod sdt;
mod smp;

use std::collections::btree_map::BTreeMap;

pub use apic::LAPIC_REGISTERS;
use fadt::Fadt;
pub use hpet::HpetTable;
pub use lapic_timer::{ScheduledEvent, cancel_scheduled_event, schedule_event};
use madt::Madt;
pub use mcfg::{BaseAddressAllocation, McfgTable};
use platform_info::PlatformInfo;
pub use smp::ap_startup::ap_startup;
pub use smp::cpu_locals;

use crate::{
    acpi::smp::cpu_init_common,
    limine::LIMINE_BOOTLOADER_REQUESTS,
    memory::{self, addresses::*},
    println, printlnc,
};

static mut PLATFORM_INFO: Option<PlatformInfo> = None;
pub static mut ACPI_TABLE_MAP: BTreeMap<&str, VirtAddr> = BTreeMap::new();

//this is safe because it's set when only 1 core is active, after that it's read only
pub fn get_table<T: 'static>(name: &str) -> Option<&T> {
    let addr = unsafe { ACPI_TABLE_MAP.get(name).copied()? };
    unsafe { Some(get_at_addr::<T, _>(addr)) }
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
    println!(level:info, "rsdt is valid");

    let tables = rsdt.get_tables();
    for table in &tables {
        unsafe {
            let table_virt: VirtAddr = (*table).into();
            let table_ptr = table_virt.0 as *const sdt::AcpiSdtHeader;
            let table_len = (table_ptr.byte_add(4) as *const u32).read_unaligned();
            let table_virt = align_manual(table_virt, table_len as u64, 8);
            let header = get_at_addr::<sdt::AcpiSdtHeader, _>(table_virt);
            let signature = std::str::from_utf8(&header.signature).expect("signatures are ascii, error in mem read");
            println!(level:info, "Found ACPI table: {} at physical address {:X}", signature, table.0);
            ACPI_TABLE_MAP.insert(signature, table_virt);
        }
    }
    println!(level:info, "Acpi tables read");
}

pub fn init_acpi() {
    let fadt = get_table::<Fadt>("FACP").expect("fadt should be present");
    let madt = get_table::<Madt>("APIC").expect("madt should be present");

    let entries = madt.get_madt_entries();
    let platform_info = platform_info::PlatformInfo::new(&entries, PhysAddr(madt.local_apic_address as u64));
    //override madt apic address if it exists in entries
    println!(level:info, "initing APIC");
    unsafe {
        PLATFORM_INFO = Some(platform_info);
    };
    let platform_info = get_platform_info();
    cpu_locals::init(platform_info);

    apic::enable_apic(platform_info, platform_info.boot_processor.processor_id);
    ioapic::init_ioapic(platform_info);

    smp::wake_cpus(platform_info);
    printlnc!(level:info, (0, 255, 0), "ACPI initialized and APs started");
    memory::unmap_lower_half();

    cpu_init_common();

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

    let _dsdt_addr = VirtAddr::from(PhysAddr(fadt.dsdt as u64));
    //let _aml_code = aml::AmlCode::new(dsdt_addr.0 as *const u8);
}

pub fn init_acpi_ap(processor_id: u8) {
    unsafe {
        let Some(platform_info) = &PLATFORM_INFO else {
            panic!("should be impossible, acpi tables are not loaded but APs were initialized");
        };
        apic::enable_apic(platform_info, processor_id);
    }

    cpu_init_common();
}
