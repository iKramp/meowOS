use core::str::FromStr;
use std::boxed::Box;
use std::error::KernelError;
use std::kerror_unwrapped;

use uuid::Uuid;

use crate::proc::CommandSplitter;
use crate::shell::AsyncCommandRetType;
use crate::vfs::{self};

//  comment: rfs: root=5f8777fa-f706-421a-9528-5364c9679890
//  comment: rfs2: root=050532EF-C9D5-4A38-A2EC-2FC3D79E5554
//  comment: fat: root=e9a75ddc-0587-45af-963f-ebbc44c99083

const RFS2_UUID: Uuid = Uuid::from_u128(0x050532EFC9D54A38A2EC2FC3D79E5554);
const FAT_UUID: Uuid = Uuid::from_u128(0xe9a75ddc058745af963febbc44c99083);

pub(super) fn cmd_mount(args: CommandSplitter) -> AsyncCommandRetType {
    Box::pin(cmd_mount_internal(args))
}

#[heap_future::heap_future]
pub async fn cmd_mount_internal(mut args: CommandSplitter) -> Result<(), KernelError> {
    let mountpoint = args.next().ok_or(kerror_unwrapped!(InvalidArgument))?;
    let part_id = args.next().ok_or(kerror_unwrapped!(InvalidArgument))?;
    let resolved_mountpoint = vfs::resolve_path(&mountpoint);
    let part_id = match part_id.as_str() {
        "rfs2" => RFS2_UUID,
        "fat" => FAT_UUID,
        _ => Uuid::from_str(&part_id).map_err(|_| kerror_unwrapped!(InvalidArgument))?,
    };

    vfs::mount_blkdev_partition(part_id, resolved_mountpoint).await
}
