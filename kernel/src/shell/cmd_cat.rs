use std::{Box, error::KernelError, kerror_unwrapped, lock_w_info, vec::Vec};

use crate::{
    memory::{
        addresses::{VirtAddr, owned_phys_slice_to_non_owned},
        physical_allocator,
    },
    proc::CommandSplitter,
    shell::AsyncCommandRetType,
    tty::TTY,
    vfs::{self, file::OpenFlags},
};

const READ_CHUNK_SIZE: u64 = 4096;

//to fix lifetimes
pub(super) fn cmd_cat(args: CommandSplitter) -> AsyncCommandRetType {
    Box::pin(cmd_cat_internal(args))
}

async fn cmd_cat_internal(mut args: CommandSplitter) -> Result<(), KernelError> {
    let path = args.next().ok_or(kerror_unwrapped!(InvalidArgument))?;
    let resolved_path = vfs::resolve_path(&path);
    let file_handle = vfs::open_file((&resolved_path).into(), None, OpenFlags(1)).await?;
    let file_info = vfs::stat_file(&file_handle).await;

    let file_size = file_info.size;
    let mut remaining_size = file_size;
    let frames = READ_CHUNK_SIZE.min(file_size).div_ceil(4096) as usize;
    let mut buffer = Vec::with_capacity(frames);
    for _ in 0..(frames) {
        let frame = physical_allocator::allocate();
        buffer.push(frame);
    }

    let chunks = file_size.div_ceil(READ_CHUNK_SIZE);

    for _ in 0..chunks {
        let non_owned_buffer = owned_phys_slice_to_non_owned(&buffer);
        vfs::read_file(&file_handle, non_owned_buffer, READ_CHUNK_SIZE.min(remaining_size)).await?;
        remaining_size -= READ_CHUNK_SIZE.min(remaining_size);

        let mut read_data = 0;

        let tty = lock_w_info!(TTY);

        for frame in &buffer {
            let to_read = (file_size - read_data).min(4096);
            let virt_addr = VirtAddr::from(frame.0);
            let ptr = virt_addr.0 as *const u8;
            let data = unsafe { core::slice::from_raw_parts(ptr, to_read as usize) };
            let string = unsafe { str::from_utf8_unchecked(data) };
            tty.print(string);

            read_data += to_read;
        }
    }
    let tty = lock_w_info!(TTY);
    tty.print("\n");

    Ok(())
}
