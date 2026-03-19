pub(super) struct BlockBitmask {
    bitmask: [u64; 4096 / core::mem::size_of::<u64>()],
}

pub(super) struct InodeBtmask {
    bitmask: [u64; 4096 / core::mem::size_of::<u64>() - 1],
    next_ptr: u64,
}
pub(super) const INODES_PER_BITMASK: usize = (4096 - core::mem::size_of::<u64>()) * 8;

impl BlockBitmask {
    pub fn new() -> Self {
        Self {
            bitmask: [0; 4096 / core::mem::size_of::<u64>()],
        }
    }

    pub fn is_set(&self, index: usize) -> bool {
        let block_index = index / 64;
        let bit_index = index % 64;
        (self.bitmask[block_index] & (1 << bit_index)) != 0
    }

    pub fn set(&mut self, index: usize) {
        let block_index = index / 64;
        let bit_index = index % 64;
        self.bitmask[block_index] |= 1 << bit_index;
    }

    pub fn clear(&mut self, index: usize) {
        let block_index = index / 64;
        let bit_index = index % 64;
        self.bitmask[block_index] &= !(1 << bit_index);
    }

    pub fn find_empty(&self) -> Option<usize> {
        for (i, block) in self.bitmask.iter().enumerate() {
            if *block != u64::MAX {
                for j in 0..64 {
                    if (block & (1 << j)) == 0 {
                        return Some(i * 64 + j);
                    }
                }
            }
        }
        None
    }
}

impl InodeBtmask {
    pub fn new() -> Self {
        Self {
            bitmask: [0; 4096 / core::mem::size_of::<u64>() - 1],
            next_ptr: 0,
        }
    }

    pub fn set(&mut self, index: usize) {
        let block_index = index / 64;
        let bit_index = index % 64;
        self.bitmask[block_index] |= 1 << bit_index;
    }

    pub fn clear(&mut self, index: usize) {
        let block_index = index / 64;
        let bit_index = index % 64;
        self.bitmask[block_index] &= !(1 << bit_index);
    }

    pub fn get(&self, index: usize) -> bool {
        let block_index = index / 64;
        let bit_index = index % 64;
        (self.bitmask[block_index] & (1 << bit_index)) != 0
    }

    pub fn find_empty(&self) -> Option<usize> {
        for (i, block) in self.bitmask.iter().enumerate() {
            if *block != u64::MAX {
                for j in 0..64 {
                    if (block & (1 << j)) == 0 {
                        return Some(i * 64 + j);
                    }
                }
            }
        }
        None
    }

    pub fn get_ptr(&self) -> u64 {
        self.next_ptr
    }

    pub fn set_ptr(&mut self, ptr: u64) {
        self.next_ptr = ptr;
    }
}
