use std::{error::KernelError, format, lock_w_info};

use crate::{memory::physical_allocator, proc::CommandSplitter, tty::TTY};

pub(super) fn cmd_pmstat(_args: CommandSplitter) -> Result<(), KernelError> {
    let stat = physical_allocator::stat();
    lock_w_info!(TTY).print(&format!("Physical Memory Allocator Stats:\n{:#?}\n", stat));
    Ok(())
}
