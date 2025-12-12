use core::{
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use std::{
    boxed::Box, collections::btree_map::BTreeMap, lock_w_info, sync::arc::Arc, sync::no_int_spinlock::NoIntSpinlock, vec::Vec,
};

use crate::{
    acpi::cpu_locals::CpuLocals,
    interrupts::{disable_interrupts, enable_interrupts},
    memory::paging,
    proc::{self, Pid, ProcessData, switch_to_generic_mem_tree},
};

fn nop(_: *const ()) {}
fn nop_clone(_: *const ()) -> RawWaker {
    RawWaker::new(core::ptr::null(), &RawWakerVTable::new(nop_clone, nop, nop, nop))
}
fn nop_waker() -> Waker {
    // SAFETY: VTABLE functions are no-ops, so this is safe
    unsafe {
        static VTABLE: RawWakerVTable = RawWakerVTable::new(nop_clone, nop, nop, nop);
        Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE))
    }
}

pub fn block_task<'a, T>(mut task: Pin<Box<dyn Future<Output = T> + 'a>>) -> T {
    let locals = CpuLocals::get();
    let before_blocking = locals.lock_info.is_blocking_task();
    locals.lock_info.blocking_task();
    let data = loop {
        match task.as_mut().poll(&mut Context::from_waker(&nop_waker())) {
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
pub type AsyncTask = Pin<Box<dyn Future<Output = ()>>>;
struct AsyncTaskInternal {
    task: AsyncTask,
    proc_id: Option<Pid>,
    id: u64,
}
struct AsyncTaskHolder {
    task: AsyncTaskInternal,
    next_task: Option<Box<AsyncTaskHolder>>,
}

pub struct AsyncTaskData {
    task_id_counter: u64,
    task_list: Option<Box<AsyncTaskHolder>>,
    tasks_to_wake: NoIntSpinlock<Vec<u64>>,
    waiting_tasks: NoIntSpinlock<BTreeMap<u64, AsyncTaskInternal>>,
}

impl AsyncTaskData {
    pub const fn new() -> Self {
        Self {
            task_id_counter: 0,
            task_list: None,
            tasks_to_wake: NoIntSpinlock::new(Vec::new()),
            waiting_tasks: NoIntSpinlock::new(BTreeMap::new()),
        }
    }
}

pub fn yield_now() -> YieldOnce {
    YieldOnce { yielded: false }
}

pub struct YieldOnce {
    yielded: bool,
}

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pub fn add_task(task: AsyncTask, pid: Option<Pid>) {
    let locals = CpuLocals::get();
    let interrupts = disable_interrupts();

    let task_data = &mut locals.async_task_data;
    let id = task_data.task_id_counter;
    task_data.task_id_counter += 1;

    let task = AsyncTaskHolder {
        task: AsyncTaskInternal { task, id, proc_id: pid },
        next_task: locals.async_task_data.task_list.take(),
    };
    locals.async_task_data.task_list = Some(Box::new(task));
    if interrupts {
        enable_interrupts();
    }
}

pub fn wake_task(task_id: u64, apic_id: u8) {
    let locals = CpuLocals::get();
    let locals_start = locals.self_addr.0 - (locals.apic_id as u64 * core::mem::size_of::<CpuLocals>() as u64);
    let target_locals = locals_start + (apic_id as u64 * core::mem::size_of::<CpuLocals>() as u64);
    let locals = unsafe { &mut *(target_locals as *mut CpuLocals) };

    let mut to_wake = lock_w_info!(locals.async_task_data.tasks_to_wake);
    to_wake.push(task_id);
}

fn sleep_task(task: AsyncTaskInternal) {
    let locals = CpuLocals::get();
    let mut waiting_lock = lock_w_info!(locals.async_task_data.waiting_tasks);
    waiting_lock.insert(task.id, task);
}

fn wake_tasks_in_list() {
    let locals = CpuLocals::get();
    let mut wake_lock = lock_w_info!(locals.async_task_data.tasks_to_wake);
    let to_wake = core::mem::take(&mut *wake_lock);
    drop(wake_lock);

    if !to_wake.is_empty() {
        let mut waiting_lock = lock_w_info!(locals.async_task_data.waiting_tasks);
        for task_id in to_wake {
            if let Some(task) = waiting_lock.remove(&task_id) {
                let task = AsyncTaskHolder {
                    task,
                    next_task: locals.async_task_data.task_list.take(),
                };
                locals.async_task_data.task_list = Some(Box::new(task));
            }
        }
        drop(waiting_lock);
    }
}

pub fn process_tasks() {
    let locals = CpuLocals::get();
    locals.lock_info.assert_no_locks();
    wake_tasks_in_list();
    let interrupts = disable_interrupts();
    let mut tasks_to_process = core::mem::take(&mut locals.async_task_data.task_list);
    if interrupts {
        enable_interrupts();
    }

    let current_proc = None;

    while let Some(mut task) = tasks_to_process {
        tasks_to_process = task.next_task.take();
        let new_pid = task.task.proc_id;
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
        switch_mem_tree(&mut current_proc.as_ref(), proc.as_ref());
        process_single_task(*task);
    }
    switch_to_generic_mem_tree();
}

fn w_clone(this_data: *const ()) -> RawWaker {
    let data = unsafe { Box::from_raw(this_data as *mut WakerData) };
    let cloned = WakerData {
        apic_id: data.apic_id,
        task_id: data.task_id,
    };
    std::mem::forget(data); //avoid double free
    row_raw_waker(Box::new(cloned))
}
fn w_wake(this_data: *const ()) {
    let data = unsafe { Box::from_raw(this_data as *mut WakerData) };
    wake_task(data.task_id, data.apic_id);
}
fn w_wake_by_ref(this_data: *const ()) {
    let data = unsafe { &*(this_data as *const WakerData) };
    wake_task(data.task_id, data.apic_id);
}
fn w_drop(this_data: *const ()) {
    let _data = unsafe { Box::from_raw(this_data as *mut WakerData) };
}

struct WakerData {
    apic_id: u8,
    task_id: u64,
}

fn row_raw_waker(data: Box<WakerData>) -> RawWaker {
    let ptr = Box::into_raw(data) as *const ();
    static VTABLE: RawWakerVTable = RawWakerVTable::new(w_clone, w_wake, w_wake_by_ref, w_drop);
    RawWaker::new(ptr, &VTABLE)
}
fn ros_waker(data: Box<WakerData>) -> Waker {
    // SAFETY: VTABLE functions are no-ops, so this is safe
    unsafe { Waker::from_raw(row_raw_waker(data)) }
}

fn process_single_task(mut task: AsyncTaskHolder) {
    let locals = CpuLocals::get();
    let waker_data = Box::new(WakerData {
        apic_id: locals.apic_id,
        task_id: task.task.id,
    });
    let result = task.task.task.as_mut().poll(&mut Context::from_waker(&ros_waker(waker_data)));

    match result {
        Poll::Pending => {
            sleep_task(task.task);
        }
        Poll::Ready(_) => {}
    }
}

fn switch_mem_tree<'a>(old_proc: &mut Option<&'a Arc<ProcessData>>, new_proc: Option<&'a Arc<ProcessData>>) {
    if let Some(new) = new_proc {
        let addr = new.get().page_tree().root();
        paging::PageTree::set_level4_addr(addr);
    } else {
        proc::switch_to_generic_mem_tree();
    }

    *old_proc = new_proc;
}
