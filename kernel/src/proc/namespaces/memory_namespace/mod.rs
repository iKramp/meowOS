use std::{mem_utils::PhysAddr, sync::arc::Arc, vec::Vec};

use crate::memory::VirtualMemoryRange;

pub(in crate::proc) struct MemoryNamespace {
    internal: Arc<UniqueMemoryNamespace>,
}

struct UniqueMemoryNamespace {
    page_tree_root: PhysAddr,
    memory_ranges: Vec<Arc<VirtualMemoryRange>>,
}
