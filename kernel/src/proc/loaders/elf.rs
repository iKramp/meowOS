use std::{mem_utils::VirtAddr, println, vec::Vec};

use crate::{
    parsers::elf,
    proc::context::info::{ContextInfo, MemoryRegionDescriptor, MemoryRegionFlags},
};

use super::{ProcessLoadError, ProcessLoader};

pub(super) fn proc_loader() -> ProcessLoader {
    ProcessLoader {
        is_this_type: is_elf,
        load_context: load_elf_process,
    }
}

fn is_elf(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == b"\x7fELF"
}

fn load_elf_process<'a>(data: &'a [u8], cmdline: &'a str) -> Result<ContextInfo<'a>, super::ProcessLoadError> {
    let parsed_elf = if (data.as_ptr() as usize).is_multiple_of(8) {
        elf::parse(data)
    } else {
        panic!("data into load_elf_process must be u64 aligned");
    };

    let parsed_elf = match parsed_elf {
        Ok(elf) => elf,
        Err(e) => {
            return match e {
                elf::ParseError::InvalidMagic => Err(ProcessLoadError::InvalidFile),
                elf::ParseError::InvalidClass => Err(ProcessLoadError::UnsupportedProcessFormat),
                elf::ParseError::InvalidDataEncoding => Err(ProcessLoadError::UnsupportedProcessFormat),
                elf::ParseError::InvalidVersion => Err(ProcessLoadError::InvalidFile),
                elf::ParseError::IncompleteData => Err(ProcessLoadError::UnparseableFile),
                elf::ParseError::InvalidData => Err(ProcessLoadError::UnparseableFile),
                elf::ParseError::Other => Err(ProcessLoadError::UnparseableFile),
            };
        }
    };

    let mut regions = Vec::new();
    let mut regions_init = Vec::new();

    for (segment, segment_data) in parsed_elf.segments.iter() {
        if segment.p_type != elf::PType::PT_LOAD as u32 {
            continue; // Only loadable segments
        }

        println!(
            "Processing segment with vaddr {:x?}, memsz {:x?}, filesz {:x?}, p_flags {:x?}",
            segment.p_vaddr, segment.p_memsz, segment.p_filesz, segment.p_flags
        );
        let start = segment.p_vaddr & (!0xfff); // Align to page boundary
        let start_extended = segment.p_vaddr - start;
        let size = segment.p_memsz as usize + start_extended as usize;
        let mut flags = MemoryRegionFlags(0);
        flags.set_is_writeable(segment.p_flags.write());
        flags.set_is_executable(segment.p_flags.execute());
        let region = MemoryRegionDescriptor::new(VirtAddr(start), size.div_ceil(0x1000), flags);
        match region {
            Err(_e) => {
                return Err(ProcessLoadError::InvalidFile);
            }
            Ok(region) => {
                regions.push(region);
            }
        }
        if !segment_data.is_empty() {
            let init_region = (VirtAddr(start + start_extended), *segment_data);
            regions_init.push(init_region);
        }
    }

    if regions.is_empty() {
        return Err(ProcessLoadError::InvalidFile);
    }
    println!("rip will be set to {:#x}", parsed_elf.header.e_entry);
    let context_info = ContextInfo::new(
        false,
        &mut regions,
        regions_init.into_boxed_slice(),
        VirtAddr(parsed_elf.header.e_entry),
        cmdline,
    );

    context_info.map_err(|_| ProcessLoadError::InvalidFile)
}
