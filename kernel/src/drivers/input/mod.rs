use core::{
    sync::atomic::AtomicU64,
    task::{Poll, Waker},
};
use std::{
    alloc::{
        boxed::Box,
        string::{String, ToString},
        sync::Arc,
        vec::Vec,
    },
    error::KernelError,
    format, lock_w_info, r_lock_w_info,
    sync::{no_int_spinlock::NoIntSpinlock, rw_lock::RWSpinlock},
    time::FfiInstant,
    w_lock_w_info,
};

pub mod evnet_codes;
pub mod input_adapter;

const MAX_QUEUED_EVENTS: usize = 64;
pub static INPUT_EVENT_MANAGER: InputEventManager = InputEventManager::new();

pub(super) fn init_input() {
    //nothing yet...
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Controller,
    Touchpad,
    _6Dof,
    Other(u8),
}
const ALL_VARIANTS: [InputDeviceType; 5] = [
    InputDeviceType::Keyboard,
    InputDeviceType::Mouse,
    InputDeviceType::Controller,
    InputDeviceType::Touchpad,
    InputDeviceType::_6Dof,
];

type DeviceId = u64;
type EventBufferWithType = (Option<InputDeviceType>, Option<DeviceId>, EventBufferWrapper);

pub struct InputEventManager {
    queues: RWSpinlock<Vec<EventBufferWithType>>,
    queue_counter: AtomicU64,
}

impl InputEventManager {
    pub const fn new() -> Self {
        Self {
            queues: RWSpinlock::new(Vec::new()),
            queue_counter: AtomicU64::new(0),
        }
    }

    pub fn buffers_exist(&self) -> bool {
        let queues = r_lock_w_info!(self.queues);
        !queues.is_empty()
    }

    pub fn add_events(&self, device: Option<InputDeviceType>, device_id: Option<DeviceId>, events: &[InputEvent]) {
        let queues = r_lock_w_info!(self.queues);
        for queue in queues.iter() {
            let dt_matches = if let Some(dt1) = &queue.0
                && let Some(dt2) = &device
            {
                dt1 == dt2
            } else {
                true
            };
            let did_matches = if let Some(did1) = &queue.1
                && let Some(did2) = &device_id
            {
                did1 == did2
            } else {
                true
            };
            if dt_matches && did_matches {
                let _ = queue.2.add_events(events); //extra events are dropped
            }
        }
    }

    pub fn consume_events(&self, buffer_id: u64, buffer: &mut [InputEvent]) -> usize {
        let queues = r_lock_w_info!(self.queues);
        for queue in queues.iter() {
            if queue.2.id == buffer_id {
                return queue.2.consume_events(buffer);
            }
        }
        0
    }

    pub async fn consume_events_async(&self, buffer_id: u64, buffer: &mut [InputEvent]) -> usize {
        if buffer.is_empty() {
            return 0;
        }
        let mut reader = None;
        let queues = r_lock_w_info!(self.queues);
        for queue in queues.iter() {
            if queue.2.id == buffer_id {
                reader = Some(queue.2.consume_event_async());
                break;
            }
        }
        drop(queues);
        let Some(reader) = reader else {
            return 0;
        };
        let first_event = reader.await.ok();
        let mut count = 0;
        if let Some(event) = first_event {
            buffer[count] = event;
            count += 1;
        } else {
            return count;
        }
        count += self.consume_events(buffer_id, &mut buffer[count..]);
        count
    }

    pub fn add_buffer(&self, device: Option<InputDeviceType>, device_id: Option<DeviceId>) -> u64 {
        let new_id = self.queue_counter.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let new_queue = EventBufferWrapper {
            id: new_id,
            inner: Arc::new(NoIntSpinlock::new(EventBuffer::new())),
        };
        let mut queues = w_lock_w_info!(self.queues);
        queues.push((device, device_id, new_queue));
        new_id
    }

    pub fn remove_buffer(&self, buffer_id: u64) {
        let mut queues = w_lock_w_info!(self.queues);
        queues.retain(|e| e.2.id != buffer_id);
    }

    pub fn get_buffer_type(&self, buffer_id: u64) -> Option<(Option<InputDeviceType>, Option<DeviceId>)> {
        let queues = r_lock_w_info!(self.queues);
        for queue in queues.iter() {
            if queue.2.id == buffer_id {
                return Some((queue.0, queue.1));
            }
        }
        None
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InputEvent {
    pub generated_at: FfiInstant,
    pub device_type: u16,
    pub code: u16,
    pub value: u16,
}

struct EventBufferWrapper {
    id: u64,
    inner: Arc<NoIntSpinlock<EventBuffer>>,
}

struct EventBuffer {
    queue: Box<[InputEvent; MAX_QUEUED_EVENTS]>,
    async_waker: Option<Waker>,
    first_valid_event: u32,
    first_empty_space: u32,
}

impl EventBufferWrapper {
    pub fn add_events(&self, events: &[InputEvent]) -> Result<(), KernelError> {
        let mut inner = lock_w_info!(self.inner);
        inner.add_events(events)
    }

    pub fn consume_events(&self, buffer: &mut [InputEvent]) -> usize {
        let mut inner = lock_w_info!(self.inner);
        inner.consume_events(buffer)
    }

    pub fn consume_event_async(&self) -> AsyncEventReader {
        AsyncEventReader {
            event_buffer: self.inner.clone(),
        }
    }

    pub async fn consume_events_async(&self, buffer: &mut [InputEvent]) -> usize {
        let reader = AsyncEventReader {
            event_buffer: self.inner.clone(),
        };
        let mut count = 0;
        let first_event = reader.await.ok();
        if let Some(event) = first_event {
            buffer[count] = event;
            count += 1;
        } else {
            return count;
        }
        count += self.consume_events(&mut buffer[count..]);
        count
    }
}

impl EventBuffer {
    pub fn new() -> Self {
        Self {
            queue: Box::new([InputEvent::default(); MAX_QUEUED_EVENTS]),
            async_waker: None,
            first_valid_event: 0,
            first_empty_space: 0,
        }
    }

    //max queued events is actually 1 less
    pub fn add_events(&mut self, events: &[InputEvent]) -> Result<(), KernelError> {
        let available_space = {
            let first_empty_space = self.first_empty_space as i64;
            let mut first_valid_event = self.first_valid_event as i64;
            if first_valid_event < first_empty_space {
                first_valid_event += MAX_QUEUED_EVENTS as i64;
            }
            first_valid_event - first_empty_space - 1
        };
        let events_to_add = events.len().min(available_space as usize);

        for i in 0..events_to_add {
            let empty_space_index = (self.first_empty_space + i as u32) % MAX_QUEUED_EVENTS as u32;
            self.queue[empty_space_index as usize] = events[i];
        }
        self.first_empty_space = (self.first_empty_space + events_to_add as u32) % MAX_QUEUED_EVENTS as u32;
        if let Some(waker) = self.async_waker.take() {
            waker.wake();
        }
        Ok(())
    }

    pub fn consume_event(&mut self) -> Option<InputEvent> {
        if self.first_empty_space == self.first_valid_event {
            return None;
        }
        let event_index = self.first_valid_event;
        self.first_valid_event = (self.first_valid_event + 1) % MAX_QUEUED_EVENTS as u32;
        Some(self.queue[event_index as usize])
    }

    pub fn consume_events(&mut self, buffer: &mut [InputEvent]) -> usize {
        let mut count = 0;
        while count < buffer.len() {
            if let Some(event) = self.consume_event() {
                buffer[count] = event;
                count += 1;
            } else {
                break;
            }
        }
        count
    }
}

struct AsyncEventReader {
    event_buffer: Arc<NoIntSpinlock<EventBuffer>>,
}

impl Future for AsyncEventReader {
    type Output = Result<InputEvent, KernelError>;

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let mut inner = lock_w_info!(self.event_buffer);
        let res = inner.consume_event();
        if let Some(res) = res {
            return Poll::Ready(Ok(res));
        }
        inner.async_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl From<&InputDeviceType> for u16 {
    fn from(dt: &InputDeviceType) -> Self {
        match dt {
            InputDeviceType::Keyboard => 0,
            InputDeviceType::Mouse => 1,
            InputDeviceType::Controller => 2,
            InputDeviceType::Touchpad => 3,
            InputDeviceType::_6Dof => 4,
            InputDeviceType::Other(v) => ((*v as u16) << 8) | 0xFF,
        }
    }
}

impl From<u16> for InputDeviceType {
    fn from(value: u16) -> Self {
        match value {
            0 => InputDeviceType::Keyboard,
            1 => InputDeviceType::Mouse,
            2 => InputDeviceType::Controller,
            3 => InputDeviceType::Touchpad,
            4 => InputDeviceType::_6Dof,
            v => InputDeviceType::Other((v >> 8) as u8),
        }
    }
}

impl From<&InputDeviceType> for (String, u16) {
    fn from(dt: &InputDeviceType) -> Self {
        match dt {
            InputDeviceType::Keyboard => ("keyboard".to_string(), dt.into()),
            InputDeviceType::Mouse => ("mouse".to_string(), dt.into()),
            InputDeviceType::Controller => ("controller".to_string(), dt.into()),
            InputDeviceType::Touchpad => ("touchpad".to_string(), dt.into()),
            InputDeviceType::_6Dof => ("6dof".to_string(), dt.into()),
            InputDeviceType::Other(v) => (format!("other_{}", v), dt.into()),
        }
    }
}

impl InputDeviceType {
    pub fn all_variants() -> &'static [InputDeviceType] {
        &ALL_VARIANTS
    }
}
