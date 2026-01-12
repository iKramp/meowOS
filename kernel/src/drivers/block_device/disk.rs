use core::fmt::Debug;
use std::{boxed::Box, mem_utils::PhysAddr, string::String, vec::Vec};

use uuid::Uuid;

use crate::vfs::{DeviceId, InodeIndex};

#[async_trait::async_trait]
pub trait BlockDevice: Debug + Send + Sync {
    async fn read(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]);
    async fn write(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]);
}

#[async_trait::async_trait]
pub trait PartitionSchemeDriver {
    async fn guid(&self, disk: &mut dyn BlockDevice) -> Uuid;
    ///returns a vector of partition guids (not filesystem ids) and partition objects
    async fn partitions(&self, disk: &mut dyn BlockDevice) -> Vec<(Uuid, Partition)>;
}

//Make sure this is ALWAYS send + sync
#[derive(Debug)]
pub struct MountedPartition {
    pub disk: &'static dyn BlockDevice,
    pub partition: Partition,
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub fs_type: Uuid,
    pub device: DeviceId,
    pub start_sector: usize,
    pub size_sectors: usize,
    pub name: String,
}

impl MountedPartition {
    pub fn new(disk: &'static dyn BlockDevice, partition: Partition) -> Self {
        Self { disk, partition }
    }

    pub async fn read(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]) {
        assert!(sector + sec_count <= self.partition.size_sectors);
        self.disk.read(self.partition.start_sector + sector, sec_count, buffer).await;
    }

    pub async fn write(&self, sector: usize, sec_count: usize, buffer: &[PhysAddr]) {
        assert!(sector + sec_count <= self.partition.size_sectors);
        self.disk.write(self.partition.start_sector + sector, sec_count, buffer).await;
    }
}

#[derive(Debug)]
pub struct DirEntry {
    pub inode: InodeIndex,
    pub name: Box<str>,
}
