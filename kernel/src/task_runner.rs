use core::{future::Future as StdFuture, pin::Pin, sync::atomic::AtomicU64, task::Context, task::Poll as StdPoll};
use std::{
    boxed::Box,
    ffi_future::future::{Future, Poll},
    ffi_future::wake::*,
    lock_w_info, println,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock, rw_lock::RWSpinlock},
    vec::Vec,
    w_lock_w_info,
};

use crate::{
    acpi::cpu_locals::CpuLocals,
    memory,
    proc::{self, Pid, ProcessData},
};

static REPEATING_TASKS: RWSpinlock<Vec<fn()>> = RWSpinlock::new(Vec::new());

pub fn add_repeating_task(task: fn()) {
    let mut tasks = w_lock_w_info!(REPEATING_TASKS);
    tasks.push(task);
}

pub fn run_repeating_tasks() {
    let tasks = w_lock_w_info!(REPEATING_TASKS);
    for task in tasks.iter() {
        task();
    }
}

pub fn block_task<T>(task: Future<T>) -> T {
    let mut locals = CpuLocals::get_mut();
    let before_blocking = locals.lock_info.is_blocking_task();
    locals.lock_info.blocking_task();
    let data = loop {
        match unsafe { (task.poll_fn)(task.data, Waker::noop()) } {
            Poll::Ready(data) => break data,
            Poll::Pending => {}
        }
    };
    if !before_blocking {
        locals.lock_info.unblocking_task();
    }
    data
}

//probably won't change return type, tasks should modify process state or other things themselves (through
//a pointer)
pub type AsyncTask = Future<()>;
struct AsyncTaskWrapper {
    task: AsyncTask,
    proc_id: Option<Pid>,
    id: u64,
}

impl core::fmt::Debug for AsyncTaskWrapper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FfiSafeAsyncTaskInternal")
            .field("proc_id", &self.proc_id)
            .field("id", &self.id)
            .finish()
    }
}

pub struct AsyncTaskData {
    task_id_counter: AtomicU64,
    task_list: NoIntSpinlock<std::queue::DataQueueHead<AsyncTaskWrapper>>,
    tasks_to_wake: NoIntSpinlock<Vec<u64>>,
    // waiting_tasks: NoIntSpinlock<BTreeMap<u64, AsyncTaskInternal>>,
    waiting_tasks: NoIntSpinlock<Vec<(u64, AsyncTaskWrapper)>>,
}

impl AsyncTaskData {
    pub const fn new() -> Self {
        Self {
            task_id_counter: AtomicU64::new(0),
            task_list: NoIntSpinlock::new(std::queue::DataQueueHead::new(100000)),
            tasks_to_wake: NoIntSpinlock::new(Vec::new()),
            // waiting_tasks: NoIntSpinlock::new(BTreeMap::new()),
            waiting_tasks: NoIntSpinlock::new(Vec::new()),
        }
    }
}

pub fn yield_now() -> YieldOnce {
    YieldOnce { yielded: false }
}

pub struct YieldOnce {
    yielded: bool,
}

impl StdFuture for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> StdPoll<Self::Output> {
        if self.yielded {
            StdPoll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            StdPoll::Pending
        }
    }
}

#[repr(C)]
pub enum PidOption {
    None,
    Some(Pid),
}

pub extern "C" fn add_task(task: AsyncTask, pid: PidOption) {
    let locals = CpuLocals::get();

    let id = locals
        .async_task_data
        .task_id_counter
        .fetch_add(1, core::sync::atomic::Ordering::AcqRel);

    let pid = match pid {
        PidOption::None => None,
        PidOption::Some(pid) => Some(pid),
    };

    let internal = AsyncTaskWrapper { task, proc_id: pid, id };

    let mut task_list = lock_w_info!(locals.async_task_data.task_list);
    task_list.push(internal);
}

pub fn wake_task(task_id: u64, apic_id: u8) {
    assert_eq!(apic_id, 0);
    let locals = CpuLocals::get();
    let locals_start = locals.self_addr.0 - (locals.apic_id as u64 * core::mem::size_of::<CpuLocals>() as u64);
    let target_locals = locals_start + (apic_id as u64 * core::mem::size_of::<CpuLocals>() as u64);
    let locals = unsafe { &mut *(target_locals as *mut CpuLocals) };

    let mut to_wake = lock_w_info!(locals.async_task_data.tasks_to_wake);
    to_wake.push(task_id);
}

fn sleep_task(task: AsyncTaskWrapper) {
    let locals = CpuLocals::get();
    let mut waiting_lock = lock_w_info!(locals.async_task_data.waiting_tasks);
    // waiting_lock.insert(task.id, task);
    waiting_lock.push((task.id, task));
    drop(waiting_lock);
    drop(locals);
}

fn wake_tasks_in_list(locals: &mut CpuLocals) {
    let mut wake_lock = lock_w_info!(locals.async_task_data.tasks_to_wake);
    let to_wake = core::mem::take(&mut *wake_lock);
    drop(wake_lock);

    if !to_wake.is_empty() {
        let mut task_list = lock_w_info!(locals.async_task_data.task_list);
        let mut waiting_lock = lock_w_info!(locals.async_task_data.waiting_tasks);
        for task_id in to_wake {
            if let Some(pos) = waiting_lock.iter().position(|(id, _)| *id == task_id) {
                let (_, task) = waiting_lock.remove(pos);
                task_list.push(task);
            } else {
                println!("Tried to wake non-existing async task {}", task_id);
            }
        }
        drop(waiting_lock);
        drop(task_list);
    }
}

pub fn process_tasks() {
    let mut locals = CpuLocals::get_mut();
    locals.lock_info.assert_no_locks();
    wake_tasks_in_list(&mut locals);
    drop(locals);

    loop {
        let locals = CpuLocals::get_mut();
        let mut task_list = lock_w_info!(locals.async_task_data.task_list);
        let Some(task) = task_list.get_first() else {
            break;
        };
        drop(task_list);
        drop(locals);
        let new_pid = task.proc_id;
        let proc;
        if let Some(pid) = new_pid {
            let tmp_proc = proc::get_proc(pid);
            if tmp_proc.is_none() {
                continue; //current task is removed because its process was killed
            }
            proc = tmp_proc;
        } else {
            proc = None;
        }
        switch_mem_tree(&mut None, proc.as_ref());
        process_single_task(task);
    }
}

extern "C" fn w_clone(this_data: *const ()) -> RawWaker {
    let data = unsafe { &*(this_data as *mut WakerData) };
    let cloned = WakerData {
        apic_id: data.apic_id,
        task_id: data.task_id,
    };
    ros_raw_waker(Box::new(cloned))
}
extern "C" fn w_wake(this_data: *const ()) {
    let data = unsafe { Box::from_raw(this_data as *mut WakerData) };
    wake_task(data.task_id, data.apic_id);
}
extern "C" fn w_wake_by_ref(this_data: *const ()) {
    let data = unsafe { &*(this_data as *const WakerData) };
    wake_task(data.task_id, data.apic_id);
}
extern "C" fn w_drop(this_data: *const ()) {
    let _data = unsafe { Box::from_raw(this_data as *mut WakerData) };
}

struct WakerData {
    apic_id: u8,
    task_id: u64,
}

fn ros_raw_waker(data: Box<WakerData>) -> RawWaker {
    let ptr = Box::into_raw(data) as *const ();
    static VTABLE: RawWakerVTable = RawWakerVTable::new(w_clone, w_wake, w_wake_by_ref, w_drop);
    RawWaker::new(ptr, &VTABLE)
}
fn ros_waker(data: Box<WakerData>) -> Waker {
    unsafe { Waker::from_raw(ros_raw_waker(data)) }
}

fn process_single_task(task: AsyncTaskWrapper) {
    let waker_data = Box::new(WakerData {
        apic_id: CpuLocals::get().apic_id,
        task_id: task.id,
    });
    // let result = task.task.as_mut().poll(&mut Context::from_waker(&ros_waker(waker_data)));
    let result = unsafe { (task.task.poll_fn)(task.task.data, &ros_waker(waker_data)) };

    match result {
        Poll::Pending => {
            sleep_task(task);
        }
        Poll::Ready(_) => {}
    }
}

fn switch_mem_tree<'a>(old_proc: &mut Option<&'a Arc<ProcessData>>, new_proc: Option<&'a Arc<ProcessData>>) {
    if let Some(new) = new_proc {
        let addr = new.get().page_tree();
        memory::set_cr3(addr);
    } else {
        proc::switch_to_generic_mem_tree();
    }

    *old_proc = new_proc;
}
