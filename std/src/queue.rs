use alloc::boxed::Box;

pub struct DataQueueHead<T> {
    first_node: Option<Box<DataQueueNode<T>>>,
    last_node: Option<*mut DataQueueNode<T>>,
    max_nodes: usize,
    curr_nodes: usize,
}

pub struct DataQueueNode<T> {
    next_node: Option<Box<DataQueueNode<T>>>,
    data: T,
}

unsafe impl<T> Send for DataQueueHead<T> {} //only accessed through these functions

impl<T> DataQueueHead<T> {
    pub const fn new(max_nodes: usize) -> Self {
        Self {
            first_node: None,
            last_node: None,
            max_nodes,
            curr_nodes: 0,
        }
    }

    pub fn push(&mut self, data: T) {
        let mut data = Box::new(DataQueueNode {
            next_node: None,
            data,
        });

        while self.curr_nodes >= self.max_nodes {
            //drop oldest
            let _ = self.get_first();
        }

        let Some(last_node_ptr) = self.last_node else {
            let raw_ptr = data.as_mut() as *mut DataQueueNode<T>;
            self.first_node = Some(data);
            self.last_node = Some(raw_ptr);
            self.curr_nodes = 1;
            return;
        };

        let last_node = unsafe { &mut *last_node_ptr };
        let new_raw_ptr = data.as_mut() as *mut DataQueueNode<T>;
        last_node.next_node = Some(data);
        self.last_node = Some(new_raw_ptr);
        self.curr_nodes += 1;
    }

    pub fn get_first(&mut self) -> Option<T> {
        let mut dummy = Option::None;
        core::mem::swap(&mut dummy, &mut self.first_node);
        let mut first_node = dummy?;
        core::mem::swap(&mut first_node.next_node, &mut self.first_node);

        if self.first_node.is_none() {
            self.last_node = None;
        }

        Some(first_node.data)
    }

    pub fn append(&mut self, other: DataQueueHead<T>) {
        if other.curr_nodes == 0 {
            return;
        }

        let max_nodes = self.max_nodes;

        while self.curr_nodes + other.curr_nodes > self.max_nodes && self.curr_nodes > 0 {
            //drop oldest
            let _ = self.get_first();
        }

        if self.first_node.is_none() {
            *self = other;
            self.max_nodes = max_nodes;
            while self.curr_nodes > max_nodes {
                let _ = self.get_first();
            }
            return;
        }

        let Some(last_node_ptr) = self.last_node else {
            unreachable!();
        };

        let last_node = unsafe { &mut *last_node_ptr };
        last_node.next_node = other.first_node;
        self.last_node = other.last_node;
        self.curr_nodes += other.curr_nodes;
    }

    pub fn len(&self) -> usize {
        self.curr_nodes
    }
}
