use core::mem::MaybeUninit;
use std::{alloc::boxed::Box, error::KernelError, kerror, ring_queue::RingQueue};

use crate::{
    arch::signal::SignalCpuContext,
    interrupts::InterruptProcessorState,
    memory::{addresses::VirtAddr, safe_memcpy_from_user},
    proc::{
        Pid, ProcessData,
        signal::{
            default_handlers::{run_default_handler, run_registered_prehandler},
            linux_compat::LinSiFields,
        },
    },
};

mod default_handlers;
mod linux_compat;
mod recv_namespace_signal;

pub trait SignalFields: core::any::Any + Send {
    fn get_data_as_bytes(&self) -> &[u8];
}

#[repr(C)]
pub struct SignalData {
    pub signal_type: SignalType,
    pub source: Pid,
    pub fields: Box<dyn SignalFields>,
}

//try to match with linux
#[repr(C)]
pub struct SignalFrame<T> {
    pub retcode: VirtAddr,
    pub user_context: SignalCpuContext,
    pub signal_info: SignalInfo<T>,
}

#[repr(C)]
pub struct SignalInfo<T> {
    pub signal_number: u32,
    pub ret_error: u32,
    pub ret_code: u32,
    pub signal_fields: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SignalType {
    //For compatibility, linux signals first. Some won't be implemented likely
    Hangup = 1,
    Interrupt = 2,
    Quit = 3,
    IllegalInstruction = 4,
    Trap = 5,
    Abort = 6,
    BusError = 7,
    FloatingPointException = 8,
    Kill = 9,
    UserDefined1 = 10,
    SegmentationFault = 11,
    UserDefined2 = 12,
    Pipe = 13,
    Alarm = 14,
    Termination = 15,
    StackFault = 16,
    ChildStatusChanged = 17,
    Continue = 18,
    Stop = 19,
    TerminalStop = 20,
    TTin = 21,            //??
    TTou = 22,            //??
    UrgentCondition = 23, //urgent pointer yippie!!
    CPUTimeLimitExceeded = 24,
    FileSizeLimitExceeded = 25,
    VirtualTimerExpired = 26,
    ProfilingTimerExpired = 27,
    WindowChanged = 28,
    IO = 29,
    PowerFailure = 30, //??? WHY WOULD A PROCESS CARE ABOUT THIS??
    BadSystemCall = 31,
    //up to 64 "normal" signals

    //Special signals
    ReceivedNamespace = 64,
}

pub(super) struct SignalHandlerTable {
    pub(super) handlers: [VirtAddr; 128],
}

pub(super) struct SignalState {
    pending_linux_signals: u64, //allow for expansion
    linux_signal_data: [LinSiFields; 64],
    pending_special_signals: RingQueue<SignalData>,
    handler_table: SignalHandlerTable,
}

impl SignalState {
    pub fn new() -> Self {
        Self {
            pending_linux_signals: 0,
            linux_signal_data: [LinSiFields::default(); 64],
            pending_special_signals: RingQueue::new(32),
            handler_table: SignalHandlerTable {
                handlers: [VirtAddr(0); 128],
            },
        }
    }

    pub fn clone_from(other: &Self) -> Self {
        Self {
            pending_linux_signals: 0,
            linux_signal_data: [LinSiFields::default(); 64],
            pending_special_signals: RingQueue::new(32),
            handler_table: SignalHandlerTable {
                handlers: other.handler_table.handlers,
            },
        }
    }

    pub fn get_pending_linux_signals(&self) -> u64 {
        self.pending_linux_signals
    }

    pub fn get_to_handle(&self, proc: &ProcessData) -> Option<(VirtAddr, SignalData)> {
        let (handler, data) = self.get_to_handle_inner()?;

        match handler {
            Some(handler) => {
                run_registered_prehandler(proc, &data);
                Some((handler, data))
            }
            None => {
                run_default_handler(proc, data);
                None
            }
        }
    }

    fn get_to_handle_inner(&self) -> Option<(Option<VirtAddr>, SignalData)> {
        let pending_linux = self.pending_linux_signals;
        if pending_linux != 0 {
            let signal_index = pending_linux.trailing_zeros();
            let Ok(signal_type) = SignalType::try_from(signal_index as usize) else {
                return None;
            };

            let handler = self.handler_table.handlers[signal_index as usize];
            let signal_data = self.linux_signal_data[signal_index as usize];
            let signal_data = SignalData {
                signal_type,
                source: Pid(0), //linux signals have that in their structure when needed
                fields: Box::new(signal_data),
            };
            if handler.0 != 0 {
                return Some((Some(handler), signal_data));
            } else {
                return Some((None, signal_data)); //take default action
            }
        }

        if let Some(special_signal) = self.pending_special_signals.get_first() {
            let signal_type = special_signal.signal_type;
            if signal_type as usize >= self.handler_table.handlers.len() {
                return None; //illegal signal type
            }
            let handler = self.handler_table.handlers[signal_type as usize];
            if handler.0 != 0 {
                return Some((Some(handler), special_signal));
            } else {
                return Some((None, special_signal)); //take default action
            }
        }

        None
    }
}

impl TryFrom<usize> for SignalType {
    type Error = KernelError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SignalType::Hangup),
            2 => Ok(SignalType::Interrupt),
            3 => Ok(SignalType::Quit),
            4 => Ok(SignalType::IllegalInstruction),
            5 => Ok(SignalType::Trap),
            6 => Ok(SignalType::Abort),
            7 => Ok(SignalType::BusError),
            8 => Ok(SignalType::FloatingPointException),
            9 => Ok(SignalType::Kill),
            10 => Ok(SignalType::UserDefined1),
            11 => Ok(SignalType::SegmentationFault),
            12 => Ok(SignalType::UserDefined2),
            13 => Ok(SignalType::Pipe),
            14 => Ok(SignalType::Alarm),
            15 => Ok(SignalType::Termination),
            16 => Ok(SignalType::StackFault),
            17 => Ok(SignalType::ChildStatusChanged),
            18 => Ok(SignalType::Continue),
            19 => Ok(SignalType::Stop),
            20 => Ok(SignalType::TerminalStop),
            21 => Ok(SignalType::TTin),
            22 => Ok(SignalType::TTou),
            23 => Ok(SignalType::UrgentCondition),
            24 => Ok(SignalType::CPUTimeLimitExceeded),
            25 => Ok(SignalType::FileSizeLimitExceeded),
            26 => Ok(SignalType::VirtualTimerExpired),
            27 => Ok(SignalType::ProfilingTimerExpired),
            28 => Ok(SignalType::WindowChanged),
            29 => Ok(SignalType::IO),
            30 => Ok(SignalType::PowerFailure),
            31 => Ok(SignalType::BadSystemCall),

            64 => Ok(SignalType::ReceivedNamespace),
            _ => kerror!(IllegalValue),
        }
    }
}

pub fn signal_return_fn(cpu_data: &mut InterruptProcessorState) -> bool {
    let stack_pointer = cpu_data.get_stack_pointer();
    let user_context_ptr = stack_pointer as *const SignalCpuContext;
    let user_context = MaybeUninit::<SignalCpuContext>::uninit();
    let dst = user_context.as_ptr() as *mut SignalCpuContext;
    let res = safe_memcpy_from_user(dst as u64, user_context_ptr as u64, core::mem::size_of::<SignalCpuContext>());
    if !res {
        return false;
    }

    //might get funky when a syscall is interrupted by a signal and returned as an interrupt
    *cpu_data = InterruptProcessorState::from(unsafe { user_context.assume_init() });
    true
}
