use std::{mem_utils::PhysAddr, sync::arc::Arc};

use crate::memory::paging::PageTree;



pub(in crate::proc) struct MemoryNamespace {
    internal: Arc<UniqueMemoryNamespace>
}

struct UniqueMemoryNamespace {
    page_tree: PageTree,
}
