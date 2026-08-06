use std::error::ErrorCode;

use crate::vfs::file::FileHandle;

pub(super) async fn delete_inode(parent_inode: &FileHandle, name: &str) -> Result<(), ErrorCode> {
    return Ok(());
}

async fn delete_file(parent_inode: &FileHandle, name: &str) -> Result<(), ErrorCode> {
    todo!()
}

async fn delete_dir(parent_inode: &FileHandle, name: &str) -> Result<(), ErrorCode> {
    todo!()
}
