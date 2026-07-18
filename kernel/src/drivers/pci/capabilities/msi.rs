use std::error::ErrorCode;
use std::println;

use crate::memory::addresses::*;

use crate::{
    acpi,
    drivers::pci::{FullPciDevType, capabilities::CapAddr, legacy, port_access},
};

pub(in crate::drivers::pci) const PCI_CAP_MSI_ID: u8 = 0x5;

pub fn init_msi_interrupt(dev: &FullPciDevType, msi_irq: u8) -> Result<(), ErrorCode> {
    //disable INTx# interrupts (pins?)
    println!("Initializing MSI interrupt");
    let capabilities = &dev.get_common().capabilities;
    let msi_cap = capabilities.iter().find(|cap| cap.id == 5).ok_or(ErrorCode::NoEntry)?;

    #[allow(clippy::needless_late_init)] //bruh useless lint
    let cap_addr;
    match dev {
        FullPciDevType::Legacy(legacy_pci_device, _) => {
            println!("device is legacy pci device");
            println!("disabling legacy interrupt");
            let command = legacy::config_space::get_command(&legacy_pci_device.common.device);
            legacy::config_space::set_command(command | (1 << 10), &legacy_pci_device.common.device);
            cap_addr = CapAddr::IO(port_access::get_config_address(
                true,
                legacy_pci_device.common.device.bus,
                legacy_pci_device.common.device.device,
                legacy_pci_device.common.device.function,
                msi_cap.pointer,
            ));
        }
        FullPciDevType::Express(pcie_device, _) => {
            println!("device is pcie device");
            println!("disabling legacy interrupt");
            let mut command = pcie_device.config_space_addr.command().read();
            command.set_interrupt_disable(true);
            pcie_device.config_space_addr.command().write(command);
            let ptr = pcie_device.config_space_addr.as_ptr() as u64 + msi_cap.pointer as u64;
            cap_addr = CapAddr::Memory(VirtAddr(ptr));
        }
    }

    let first_dword = cap_addr.get_dword(0);
    let mut message_control = (first_dword >> 16) as u16;
    let is_64_capable = (message_control & (1 << 7)) != 0;
    let per_vector_masking_capable = (message_control & (1 << 8)) != 0;
    let _extedned_data_capable = (message_control & (1 << 9)) != 0;

    // //get number of requested interrupts, and allow max...?
    let requested_interrupts_power = u16::min((message_control & 0b1110) >> 1, 5);
    let requested_interrupts = 1 << requested_interrupts_power;
    println!("Requested interrupts: {}", requested_interrupts);

    println!("granting only 1 interrupt womp womp");
    message_control |= 0 << 4; //give only 1 interrupt

    let message_address = (0xFEE << 20) | ((acpi::get_platform_info().boot_processor.apic_id as u32) << 12) | (1 << 3); //destination mode physical APIC
    let message_data = msi_irq as u16;

    cap_addr.set_dword(4, message_address);
    if is_64_capable {
        cap_addr.set_dword(8, 0); //high dword of address
        cap_addr.set_dword(0xC, message_data as u32);
    } else {
        cap_addr.set_dword(8, message_data as u32);
    }

    if per_vector_masking_capable {
        let offset = if is_64_capable { 0x10 } else { 0xC };
        //disable all masks
        cap_addr.set_dword(offset, 0);
        cap_addr.set_dword(offset + 4, 0);
    }

    message_control |= 0x1; //enable MSI
    cap_addr.set_dword(0, (message_control as u32) << 16 | (first_dword & 0xFFFF));

    println!("MSI interrupt initialized on vector {}", msi_irq);

    Ok(())
}

pub fn disable_msi(dev: &FullPciDevType) -> Result<(), ErrorCode> {
    let capabilities = &dev.get_common().capabilities;
    let msi_cap = capabilities.iter().find(|cap| cap.id == 5).ok_or(ErrorCode::NoEntry)?;

    #[allow(clippy::needless_late_init)] //bruh useless lint
    let cap_addr;
    match dev {
        FullPciDevType::Legacy(legacy_pci_device, _) => {
            let command = legacy::config_space::get_command(&legacy_pci_device.common.device);
            legacy::config_space::set_command(command | (1 << 10), &legacy_pci_device.common.device);
            cap_addr = CapAddr::IO(port_access::get_config_address(
                true,
                legacy_pci_device.common.device.bus,
                legacy_pci_device.common.device.device,
                legacy_pci_device.common.device.function,
                msi_cap.pointer,
            ));
        }
        FullPciDevType::Express(pcie_device, _) => {
            let mut command = pcie_device.config_space_addr.command().read();
            command.set_interrupt_disable(true);
            pcie_device.config_space_addr.command().write(command);
            let ptr = pcie_device.config_space_addr.as_ptr() as u64 + msi_cap.pointer as u64;
            cap_addr = CapAddr::Memory(VirtAddr(ptr));
        }
    }

    let mut first_dword = cap_addr.get_dword(0);
    first_dword &= !(1 << 16);
    cap_addr.set_dword(0, first_dword); //disable

    todo!()
}

fn set_msi_address(is_64_bit: bool, cap_addr: &CapAddr) {
    let platform_info = crate::acpi::get_platform_info();
    let destination_mode = 0;
    let destination_id = platform_info.boot_processor.apic_id as u32;

    let irq_address = 0xFFE << 20 | destination_mode << 2 | destination_id << 12;

    let low_address = irq_address;
    let high_address = 0;

    cap_addr.set_dword(4, low_address);
    if is_64_bit {
        cap_addr.set_dword(8, high_address);
    }
}
