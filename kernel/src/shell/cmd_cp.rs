use std::{alloc::borrow::ToOwned, boxed::Box, error::KernelError, kerror, kerror_unwrapped, vec::Vec};

use crate::{
    memory::physical_allocator,
    proc::CommandSplitter,
    shell::AsyncCommandRetType,
    vfs::{self, file::FileHandle},
};

//to fix lifetimes
pub(super) fn cmd_cp(args: CommandSplitter) -> AsyncCommandRetType {
    Box::pin(cmd_cp_internal(args))
}

async fn cmd_cp_internal(mut args: CommandSplitter) -> Result<(), KernelError> {
    let source_path = args.next().ok_or(kerror_unwrapped!(InvalidArgument))?;
    let dest_path = args.next().ok_or(kerror_unwrapped!(InvalidArgument))?;

    let resolved_source_path = crate::vfs::resolve_path(&source_path);
    if resolved_source_path.iter().count() == 0 {
        //can't copy whole root
        return kerror!(InvalidArgument);
    }

    let resolved_dest_path = crate::vfs::resolve_path(&dest_path);

    let source_file = vfs::open_file((&resolved_source_path).into(), None, *vfs::file::OpenFlags(0).set_read(true)).await?;
    let file_name = resolved_source_path
        .get(resolved_source_path.iter().count() - 1)
        .ok_or(kerror_unwrapped!(InvalidArgument))?
        .to_owned();
    let dest_parent_file = vfs::open_file(
        (&resolved_dest_path).into(),
        None,
        *vfs::file::OpenFlags(0).set_read(true).set_write(true),
    )
    .await?;

    let mut source_path_vec = resolved_source_path.take().to_vec();
    let mut dest_path_vec = resolved_dest_path.take().to_vec();

    copy_inode(
        &source_file,
        &mut source_path_vec,
        &dest_parent_file,
        &mut dest_path_vec,
        &file_name,
    )
    .await
}

/// source: Handle of the file being copied, not a parent
#[heap_future::heap_future]
async fn copy_inode(
    source: &FileHandle,
    source_path: &mut Vec<Box<str>>,
    dest_parent: &FileHandle,
    dest_path: &mut Vec<Box<str>>,
    name: &str,
) -> Result<(), KernelError> {
    let is_dir = source.file_flags.dir();
    let src_path_len = source_path.len();
    let dest_path_len = dest_path.len();

    let result = if is_dir {
        copy_dir(source, source_path, dest_parent, dest_path, name).await
    } else {
        copy_file(source, dest_parent, dest_path, name).await
    };

    source_path.truncate(src_path_len);
    dest_path.truncate(dest_path_len);

    result
}

/// source: Handle of the file being copied, not a parent
async fn copy_dir(
    source: &FileHandle,
    source_path: &mut Vec<Box<str>>,
    dest_parent: &FileHandle,
    dest_path: &mut Vec<Box<str>>,
    name: &str,
) -> Result<(), KernelError> {
    super::cmd_rm::delete_inode(dest_parent, name).await?;

    vfs::create_file(dest_parent, name, vfs::InodeTypeAndPerms::new_dir(0o777)).await?;

    dest_path.push(name.to_owned().into_boxed_str());

    let resolved_dest_path = vfs::validate_path(dest_path)?;

    let dest_dir = vfs::open_file(
        resolved_dest_path,
        None,
        *vfs::file::OpenFlags(0).set_read(true).set_write(true),
    )
    .await?;
    let direntries = vfs::get_dir_entries(source).await?;

    for entry in direntries {
        let entry_name = entry.name;
        source_path.push(entry_name.clone());
        let resolved_src_path = vfs::validate_path(source_path)?;
        let entry_file = vfs::open_file(resolved_src_path, None, *vfs::file::OpenFlags(0).set_read(true)).await?;

        copy_inode(&entry_file, source_path, &dest_dir, dest_path, &entry_name).await?;
        source_path.pop();
    }

    Ok(())
}

async fn copy_file(
    source: &FileHandle,
    dest_parent: &FileHandle,
    dest_path: &mut Vec<Box<str>>,
    name: &str,
) -> Result<(), KernelError> {
    super::cmd_rm::delete_inode(dest_parent, name).await?;

    vfs::create_file(dest_parent, name, vfs::InodeTypeAndPerms::new_file(0o777)).await?;

    dest_path.push(name.to_owned().into_boxed_str());
    let resolved_dest_path = vfs::validate_path(dest_path)?;

    let dest_file = vfs::open_file(
        resolved_dest_path,
        None,
        *vfs::file::OpenFlags(0).set_read(true).set_write(true),
    )
    .await?;

    let buffer = physical_allocator::allocate();
    let buffer_slice = [buffer.0];

    let inode = vfs::stat_file(source).await;
    let mut total_size = inode.size;

    loop {
        if total_size == 0 {
            break;
        }

        let mut bytes_read = vfs::read_file(source, &buffer_slice[..], total_size.min(4096)).await?;
        bytes_read = bytes_read.min(total_size);

        if bytes_read == 0 {
            break;
        }

        total_size -= bytes_read;

        loop {
            let bytes_written = vfs::write_file(&dest_file, &buffer_slice[..], bytes_read).await?;
            if bytes_written == 0 {
                return kerror!(Unknown);
            }
            bytes_read -= bytes_written;
            if bytes_read == 0 {
                //very likely on first iteration
                break;
            }

            let src_ptr = (buffer.0.0 + bytes_written) as *const u8;
            let dest_ptr = buffer.0.0 as *mut u8;
            unsafe { std::ptr::copy(src_ptr, dest_ptr, bytes_read as usize) };
        }
    }

    Ok(())
}
