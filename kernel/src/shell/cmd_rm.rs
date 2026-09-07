use std::error::KernelError;

use crate::vfs::file::FileHandle;

pub(super) async fn delete_inode(_parent_inode: &FileHandle, _name: &str) -> Result<(), KernelError> {
    Ok(()) //really should be todo but cp uses this
}

async fn delete_file(_parent_inode: &FileHandle, _name: &str) -> Result<(), KernelError> {
    todo!()
}

async fn delete_dir(_parent_inode: &FileHandle, _name: &str) -> Result<(), KernelError> {
    todo!()
}
