use std::{boxed::Box, error::ErrorCode, println};

use crate::vfs::{self, InodeTypeAndPerms, ResolvedPath, file::OpenFlags};

#[heap_future::heap_future]
pub(super) async fn cmd_mkdir(dir_name: &str) -> Result<(), ErrorCode> {
    let resolved_path = vfs::resolve_path(dir_name);
    let open_flags = *OpenFlags(0).set_read(true).set_write(true);
    let create_flags = InodeTypeAndPerms::new_dir(0o777);
    let mut parent_file = vfs::open_file((&ResolvedPath::root()).into(), None, open_flags).await?;

    let len = resolved_path.iter().count();

    for i in 0..len {
        if let Ok(f) = vfs::open_file(resolved_path.index(0..(i + 1)), None, open_flags).await {
            println!("parent file exists, skipping creation");
            parent_file = f;
            continue;
        }

        vfs::create_file(&parent_file, resolved_path.get(i).expect("idk"), create_flags.clone()).await?;
        parent_file = vfs::open_file(resolved_path.index(0..(i + 1)), None, open_flags).await?;
    }

    Ok(())
}
