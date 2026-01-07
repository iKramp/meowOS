use std::{error::ErrorCode, mem_utils::VirtAddr, vec::Vec};

use crate::drivers::pci::{BarTrait, FullPciDevType, MemoryBar, capabilities::CapAddr, legacy, port_access};

pub(in crate::drivers::pci) const PCI_CAP_MSIX_ID: u8 = 0x11;

fn set_table_entry(bar: &MemoryBar, bar_off: u32, entry_index: u32, msg_addr: u64, msg_data: u32, vector_control: u32) {
    bar.write_to_bar(&((msg_addr & 0xFFFFFFFF) as u32), bar_off as u64 + (entry_index as u64) * 16);
    bar.write_to_bar(&((msg_addr >> 32) as u32), bar_off as u64 + (entry_index as u64) * 16 + 4);
    bar.write_to_bar(&msg_data, bar_off as u64 + (entry_index as u64) * 16 + 8);
    bar.write_to_bar(&vector_control, bar_off as u64 + (entry_index as u64) * 16 + 12);
}

pub fn ini_msix_interrupt(dev: FullPciDevType) -> Result<(), ErrorCode> {
    //disable INTx# interrupts (pins?)
    let capabilities;
    let bars: Vec<&MemoryBar>;
    let cap_addr;
    match dev {
        FullPciDevType::Legacy(legacy_pci_device) => {
            let command = legacy::config_space::get_command(&legacy_pci_device.device);
            legacy::config_space::set_command(command | (1 << 10), &legacy_pci_device.device);
            capabilities = &legacy_pci_device.capabilities;
            bars = legacy_pci_device
                .bars
                .iter()
                .filter_map(|bar| {
                    if let crate::drivers::pci::Bar::Memory(mem_bar) = bar {
                        Some(mem_bar)
                    } else {
                        None
                    }
                })
                .collect();
            let msix_cap = capabilities
                .iter()
                .find(|cap| cap.id == PCI_CAP_MSIX_ID)
                .ok_or(ErrorCode::NoEntry)?;
            cap_addr = CapAddr::IO(port_access::get_config_address(
                true,
                legacy_pci_device.device.bus,
                legacy_pci_device.device.device,
                legacy_pci_device.device.function,
                msix_cap.pointer,
            ))
        }
        FullPciDevType::Express(pcie_device) => {
            let mut command = pcie_device.config_space_addr.command().read();
            command.set_interrupt_disable(true);
            pcie_device.config_space_addr.command().write(command);
            capabilities = &pcie_device.capabilities;
            bars = pcie_device.bars.iter().collect();
            let msix_cap = capabilities
                .iter()
                .find(|cap| cap.id == PCI_CAP_MSIX_ID)
                .ok_or(ErrorCode::NoEntry)?;
            let ptr = pcie_device.config_space_addr.as_ptr() as u64 + msix_cap.pointer as u64;
            cap_addr = CapAddr::Memory(VirtAddr(ptr));
        }
    }

    let message_control = cap_addr.get_dword(0) >> 16;
    let table_size = (message_control & 0x3FF) + 1;
    let table_off_bir = cap_addr.get_dword(4);
    let pba_off_bir = cap_addr.get_dword(8);
    let table_bar_off = (table_off_bir & 0x7) * 4 + 0x10;
    let pba_bar_off = (pba_off_bir & 0x7) * 4 + 0x10;
    let table_offset = table_off_bir & !0x7;
    let _pba_offset = pba_off_bir & !0x7;

    let table_bar = bars
        .iter()
        .find(|bar| bar.offset_in_conf_space == (table_bar_off) as u8)
        .ok_or(ErrorCode::NoEntry)?;
    let _pba_bar = bars
        .iter()
        .find(|bar| bar.offset_in_conf_space == (pba_bar_off) as u8)
        .ok_or(ErrorCode::NoEntry)?;

    let platform_info = crate::acpi::get_platform_info();
    let destination_id = platform_info.boot_processor.apic_id as u64;
    let destination_mode = 0; //0 for physical, 1 for logical
    let redirection_hint = 0; //0 for no hint, 1 for hint

    for i in 0..table_size {
        let current_free_irq =
            crate::interrupts::idt::CUSTOM_INTERRUPT_VECTOR.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let msi_addr = (0xFFE << 20) | ((destination_id as u64) << 12) | (redirection_hint << 3) | (destination_mode << 2);

        let msi_data = current_free_irq as u32;

        set_table_entry(&table_bar, table_offset, i, msi_addr, msi_data, 0);
    }

    Ok(())
}
