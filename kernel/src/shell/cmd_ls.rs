use std::{error::ErrorCode, format, lock_w_info};

use crate::{
    tty::TTY,
    vfs::{self, file::FileFlags},
};

pub(super) async fn cmd_ls(path: &str) -> Result<(), ErrorCode> {
    let resolved_path = vfs::resolve_path(path);
    let file_handle = vfs::open_file((&resolved_path).into(), None, FileFlags::new().with_read(true)).await?;

    let res = vfs::get_dir_entries(&file_handle).await;
    vfs::close_file(file_handle).await;

    match res {
        Ok(entries) => {
            let tty = lock_w_info!(TTY);
            for entry in entries {
                tty.print(&format!("{}\n", entry.name));
            }
        }
        Err(e) => return Err(e),
    }

    Ok(())
}
