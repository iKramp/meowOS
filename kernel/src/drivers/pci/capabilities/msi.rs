use core::sync;
use std::{error::ErrorCode, mem_utils::VirtAddr, println};

use crate::drivers::pci::{FullPciDevType, capabilities::CapAddr, legacy, port_access};

pub(in crate::drivers::pci) const PCI_CAP_MSI_ID: u8 = 0x5;

fn get_msi_64_bit_capable(addr: &CapAddr) -> bool {
    let dword = addr.get_dword(0) >> 16;
    (dword & 0x80) != 0
}

pub fn init_msi_interrupt(dev: FullPciDevType) -> Result<(), ErrorCode> {
    //disable INTx# interrupts (pins?)
    let capabilities;
    let cap_addr;
    match dev {
        FullPciDevType::Legacy(legacy_pci_device) => {
            let command = legacy::config_space::get_command(&legacy_pci_device.device);
            legacy::config_space::set_command(command | (1 << 10), &legacy_pci_device.device);
            capabilities = &legacy_pci_device.capabilities;
            let msi_cap = capabilities.iter().find(|cap| cap.id == 5).ok_or(ErrorCode::NoEntry)?;
            cap_addr = CapAddr::IO(port_access::get_config_address(
                true,
                legacy_pci_device.device.bus,
                legacy_pci_device.device.device,
                legacy_pci_device.device.function,
                msi_cap.pointer,
            ))
        }
        FullPciDevType::Express(pcie_device) => {
            let mut command = pcie_device.config_space_addr.command().read();
            command.set_interrupt_disable(true);
            pcie_device.config_space_addr.command().write(command);
            capabilities = &pcie_device.capabilities;
            let msi_cap = capabilities.iter().find(|cap| cap.id == 5).ok_or(ErrorCode::NoEntry)?;
            let ptr = pcie_device.config_space_addr.as_ptr() as u64 + msi_cap.pointer as u64;
            cap_addr = CapAddr::Memory(VirtAddr(ptr));
        }
    }

    let is_64_capable = get_msi_64_bit_capable(&cap_addr);
    let first_dword = cap_addr.get_dword(0);
    let mut message_control = (first_dword >> 16) as u16;

    //get number of requested interrupts, and allow max...?
    let requested_interrupts_power = u16::min((message_control & 0b1110) >> 1, 5);
    let requested_interrupts = 1 << requested_interrupts_power;
    println!("Requested interrupts: {}", requested_interrupts);
    message_control &= !0b1110000;
    //give number of vectors
    message_control |= requested_interrupts_power << 4;

    loop {
        let mut current_free_irq = crate::interrupts::idt::CUSTOM_INTERRUPT_VECTOR.load(sync::atomic::Ordering::Relaxed);
        let old_irq = current_free_irq;
        current_free_irq += requested_interrupts - 1;
        current_free_irq &= !(requested_interrupts - 1);
        let res = crate::interrupts::idt::CUSTOM_INTERRUPT_VECTOR.compare_exchange(
            old_irq,
            current_free_irq + requested_interrupts,
            sync::atomic::Ordering::SeqCst,
            sync::atomic::Ordering::Relaxed,
        );
        if res.is_err() {
            continue;
        }
        set_msi_address(is_64_capable, &cap_addr);
        let data_dword_offset = if is_64_capable { 0xC } else { 0x8 };
        let data_dword = cap_addr.get_dword(data_dword_offset);
        cap_addr.set_dword(data_dword_offset, data_dword & 0xFFFF_0000 | current_free_irq as u32);
        break;
    }

    //enable MSI
    message_control |= 0x1;

    cap_addr.set_dword(0, (message_control as u32) << 16 | (first_dword & 0xFFFF));
    Ok(())
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
