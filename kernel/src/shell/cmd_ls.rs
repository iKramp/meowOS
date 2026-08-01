use std::{Box, error::ErrorCode, format, lock_w_info};

use crate::{
    tty::TTY,
    vfs::{self, file::OpenFlags},
};

#[heap_future::heap_future]
pub(super) async fn cmd_ls(path: &str) -> Result<(), ErrorCode> {
    let resolved_path = vfs::resolve_path(path);
    let file_handle = vfs::open_file((&resolved_path).into(), None, OpenFlags(1)).await?;

    let entries = vfs::get_dir_entries(&file_handle).await?;

    let tty = lock_w_info!(TTY);
    for entry in entries {
        tty.print(&format!("{}\n", entry.name));
    }

    Ok(())
}
