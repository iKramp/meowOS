use std::{Box, error::ErrorCode, lock_w_info, vec::Vec};

use crate::{
    memory::{addresses::VirtAddr, physical_allocator},
    proc::CommandSplitter,
    shell::AsyncCommandRetType,
    tty::TTY,
    vfs::{self, file::OpenFlags},
};

//to fix lifetimes
pub(super) fn cmd_cat(args: CommandSplitter) -> AsyncCommandRetType {
    Box::pin(cmd_cat_internal(args))
}

async fn cmd_cat_internal(mut args: CommandSplitter) -> Result<(), ErrorCode> {
    let path = args.next().ok_or(ErrorCode::InvalidArgument)?;
    let resolved_path = vfs::resolve_path(&path);
    let file_handle = vfs::open_file((&resolved_path).into(), None, OpenFlags(1)).await?;
    let file_info = vfs::stat_file(&file_handle).await;

    let file_size = file_info.size;
    let mut buffer = Vec::with_capacity(file_size.div_ceil(4096) as usize);
    for _ in 0..(file_size.div_ceil(4096) as usize) {
        let frame = physical_allocator::allocate();
        buffer.push(frame);
    }

    vfs::read_file(&file_handle, &buffer.iter().map(|e| e.0).collect::<Vec<_>>(), file_size).await?;

    let mut read_data = 0;

    let tty = lock_w_info!(TTY);

    for frame in buffer {
        let to_read = (file_size - read_data).min(4096);
        let virt_addr = VirtAddr::from(frame.0);
        let ptr = virt_addr.0 as *const u8;
        let data = unsafe { core::slice::from_raw_parts(ptr, to_read as usize) };
        let string = unsafe { str::from_utf8_unchecked(data) };
        tty.print(string);

        read_data += to_read;
    }
    tty.print("\n");

    Ok(())
}
