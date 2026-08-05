use std::error::ErrorCode;

use crate::{memory, proc::CommandSplitter};

pub(super) fn cmd_mmap(_args: CommandSplitter) -> Result<(), ErrorCode> {
    memory::print_mem_mapping();
    Ok(())
}
