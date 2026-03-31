use std::{boxed::Box, vec::Vec};

mod header;
mod program_header;
mod section;

pub use program_header::PType;

#[derive(Debug)]
pub enum ParseError {
    InvalidMagic,
    InvalidClass,
    InvalidDataEncoding,
    InvalidVersion,
    IncompleteData,
    InvalidData,
    Other,
}

//parse further if needed
#[derive(Debug)]
pub struct ParsedElf<'a> {
    pub header: &'a header::Elf64_Ehdr,
    pub segments: Box<[(&'a program_header::Elf64_Phdr, &'a [u8])]>,
}

pub fn parse<'a>(data: &'a [u8]) -> Result<ParsedElf<'a>, ParseError> {
    let header = header::Elf64_Ehdr::parse(data)?;
    let section_headers = section::get_section_table(data, header.e_shoff, header.e_shentsize, header.e_shnum)?;
    let segment_headers = program_header::get_segment_table(data, header.e_phoff, header.e_phentsize, header.e_phnum)?;
    let string_section_header = &section_headers[header.e_shstrndx as usize];
    let start_shstr = string_section_header.sh_offset as usize;
    let end_shstr = start_shstr + string_section_header.sh_size as usize;
    let _shstrtab = &data[start_shstr..end_shstr];

    let mut segments = Vec::new();
    for segment in segment_headers {
        let start = segment.p_offset as usize;
        let end = start + segment.p_filesz as usize;
        if end > data.len() {
            return Err(ParseError::IncompleteData);
        }
        let segment_data = &data[start..end];
        segments.push((segment, segment_data));
    }

    let parsed = ParsedElf {
        header,
        segments: segments.into_boxed_slice(),
    };

    Ok(parsed)
}
