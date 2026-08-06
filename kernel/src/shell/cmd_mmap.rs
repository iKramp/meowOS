use std::error::KernelError;

use crate::{memory, proc::CommandSplitter};

pub(super) fn cmd_mmap(_args: CommandSplitter) -> Result<(), KernelError> {
    memory::print_mem_mapping();
    Ok(())
}
