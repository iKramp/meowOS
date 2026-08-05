use std::{Box, error::ErrorCode, format, lock_w_info, string::ToString};

use crate::{
    proc::CommandSplitter,
    shell::AsyncCommandRetType,
    tty::TTY,
    vfs::{self, file::OpenFlags},
};

//to fix lifetimes
pub(super) fn cmd_ls(args: CommandSplitter) -> AsyncCommandRetType {
    Box::pin(cmd_ls_internal(args))
}

async fn cmd_ls_internal(mut args: CommandSplitter) -> Result<(), ErrorCode> {
    let path = args.next().unwrap_or_else(|| ".".to_string());

    let resolved_path = vfs::resolve_path(&path);
    let file_handle = vfs::open_file((&resolved_path).into(), None, OpenFlags(1)).await?;

    let entries = vfs::get_dir_entries(&file_handle).await?;

    let tty = lock_w_info!(TTY);
    for entry in entries {
        tty.print(&format!("{}\n", entry.name));
    }

    Ok(())
}
