use std::error::KernelError;

use crate::vfs::file::FileHandle;

pub(super) async fn delete_inode(parent_inode: &FileHandle, name: &str) -> Result<(), KernelError> {
    return Ok(());
}

async fn delete_file(parent_inode: &FileHandle, name: &str) -> Result<(), KernelError> {
    todo!()
}

async fn delete_dir(parent_inode: &FileHandle, name: &str) -> Result<(), KernelError> {
    todo!()
}
