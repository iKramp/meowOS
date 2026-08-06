use core::{slice, str};
use std::{boxed::Box, error::KernelError, format, lock_w_info, string::ToString, vec::Vec};

use crate::{
    memory::{addresses::VirtAddr, physical_allocator},
    proc::CommandSplitter,
    shell::AsyncCommandRetType,
    tty::TTY,
    vfs::{self, file::FileHandle},
};

//to fix lifetimes
pub(super) fn cmd_tree(args: CommandSplitter) -> AsyncCommandRetType {
    Box::pin(cmd_tree_internal(args))
}

async fn cmd_tree_internal(mut args: CommandSplitter) -> Result<(), KernelError> {
    let path = args.next().unwrap_or(".".to_string());

    let tty = lock_w_info!(TTY);
    tty.print(&path);
    drop(tty);

    let resolved_path = crate::vfs::resolve_path(&path);

    let root_file = vfs::open_file((&resolved_path).into(), None, *vfs::file::OpenFlags(0).set_read(true)).await?;

    let mut path_vec = resolved_path.take().to_vec();

    tree_inode(&root_file, &mut path_vec, 0).await
}

#[heap_future::heap_future]
async fn tree_inode(file: &FileHandle, path: &mut Vec<Box<str>>, depth: usize) -> Result<(), KernelError> {
    let tty = lock_w_info!(TTY);
    tty.print(&format!("{}{}\n", "-".repeat(depth), path.last().map_or("/", |val| val)));
    drop(tty);

    let path_len = path.len();

    let is_dir = file.file_flags.dir();

    let res = if is_dir {
        tree_dir(file, path, depth + 1).await
    } else {
        tree_file(file, depth + 1).await
    };

    path.truncate(path_len);

    res
}

async fn tree_file(file: &FileHandle, depth: usize) -> Result<(), KernelError> {
    let phys_frame = physical_allocator::allocate();
    let buf = [phys_frame.0];
    let file_size = vfs::stat_file(file).await.size.min(64);
    let bytes_read = vfs::read_file(file, &buf, file_size).await?.min(file_size);
    let virt: VirtAddr = phys_frame.0.into();
    let ptr = virt.0 as *const u8;
    let str = unsafe { str::from_utf8(slice::from_raw_parts(ptr, bytes_read as usize)) };

    let tty = lock_w_info!(TTY);
    if let Ok(str) = str {
        tty.print(&format!("{}Contents: \"{}\"\n", "-".repeat(depth), str));
    } else {
        tty.print(&format!("{}Not a text file\n", "-".repeat(depth)));
    }

    Ok(())
}

async fn tree_dir(dir: &FileHandle, path: &mut Vec<Box<str>>, depth: usize) -> Result<(), KernelError> {
    let direntries = vfs::get_dir_entries(dir).await?;

    for entry in direntries {
        let entry_name = entry.name;
        path.push(entry_name);
        let resolved_path = vfs::validate_path(path)?;
        let Ok(entry_file) = vfs::open_file(resolved_path, None, *vfs::file::OpenFlags(0).set_read(true)).await else {
            let tty = lock_w_info!(TTY);
            tty.print(&format!(
                "{}Couldn't tree {}\n",
                "-".repeat(depth),
                path.last().map_or("", |val| val)
            ));
            continue;
        };

        let res = tree_inode(&entry_file, path, depth).await;

        let tty = lock_w_info!(TTY);
        if res.is_err() {
            tty.print(&format!(
                "{}Couldn't tree {}\n",
                "-".repeat(depth),
                path.last().map_or("", |val| val)
            ));
        }
        path.pop();
    }

    Ok(())
}
