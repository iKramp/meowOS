use std::println;

use crate::proc::{
    self, ProcessData,
    signal::{SignalData, SignalType, recv_namespace_signal},
};

enum DefaultAction {
    Terminate,
    DumpCore, //unimplemented
    Stop,
    Continue,
    Ignore,
}

pub(super) fn run_default_handler(proc: &ProcessData, signal_data: SignalData) {
    let term_code = signal_data.signal_type as u64 + (1 << 63);

    #[cfg(debug_assertions)]
    {
        if signal_data.signal_type == SignalType::Continue {
            let pending = proc.get_mutable().get_signal_state().get_pending_linux_signals();
            if has_stop_signals(pending) {
                println!(level:warn, "process with pid {:?} has pending stop signals while handling continue signal. This should not happen", proc.pid());
            }
        }
    }

    let action = get_default_action(signal_data.signal_type);
    match action {
        DefaultAction::Terminate => {
            proc::kill_process(proc.pid(), term_code);
        }
        DefaultAction::DumpCore => {
            println!(level:warn, "core dump not implemented, killing process");
            proc::kill_process(proc.pid(), term_code);
        }
        DefaultAction::Stop => {
            proc::sleep_process(proc.pid());
        }
        DefaultAction::Continue => {
            proc::wake_process(proc.pid());
        }
        DefaultAction::Ignore => {}
    }

    //special handlers
    #[allow(clippy::single_match)]
    match signal_data.signal_type {
        SignalType::ReceivedNamespace => {
            recv_namespace_signal::run_default_handler(proc, signal_data);
        }
        _ => {}
    }
}

pub(super) fn run_registered_prehandler(proc: &ProcessData, signal_data: &SignalData) {
    //pre-handling
    match signal_data.signal_type {
        SignalType::Continue => {
            #[cfg(debug_assertions)]
            {
                let pending = proc.get_mutable().get_signal_state().get_pending_linux_signals();
                if has_stop_signals(pending) {
                    println!(level:warn, "process with pid {:?} has pending stop signals while handling continue signal. This should not happen", proc.pid());
                }
            }

            proc::wake_process(proc.pid());
        }
        SignalType::Kill => {
            panic!("kill signal should not have a registered handler. Proc should be killed immediately");
        }
        SignalType::Stop => {
            panic!("stop signal should not have a registered handler. Proc should be stopped immediately");
        }
        _ => {}
    }
}

fn get_default_action(signal: SignalType) -> DefaultAction {
    match signal {
        SignalType::Hangup => DefaultAction::Terminate,
        SignalType::Interrupt => DefaultAction::Terminate,
        SignalType::Quit => DefaultAction::DumpCore,
        SignalType::IllegalInstruction => DefaultAction::DumpCore,
        SignalType::Trap => DefaultAction::DumpCore,
        SignalType::Abort => DefaultAction::DumpCore,
        SignalType::BusError => DefaultAction::DumpCore,
        SignalType::FloatingPointException => DefaultAction::DumpCore,
        SignalType::Kill => {
            debug_assert!(
                false,
                "kill signal should not be resolved from pending. Proc should be killed immediately"
            );
            DefaultAction::Terminate
        }
        SignalType::UserDefined1 => DefaultAction::Terminate,
        SignalType::SegmentationFault => DefaultAction::DumpCore,
        SignalType::UserDefined2 => DefaultAction::Terminate,
        SignalType::Pipe => DefaultAction::Terminate,
        SignalType::Alarm => DefaultAction::Terminate,
        SignalType::Termination => DefaultAction::Terminate,
        SignalType::StackFault => DefaultAction::Terminate,
        SignalType::ChildStatusChanged => DefaultAction::Ignore,
        SignalType::Continue => DefaultAction::Continue,
        SignalType::Stop => DefaultAction::Stop,
        SignalType::TerminalStop => DefaultAction::Stop,
        SignalType::TTin => DefaultAction::Stop,
        SignalType::TTou => DefaultAction::Stop,
        SignalType::UrgentCondition => DefaultAction::Ignore,
        SignalType::CPUTimeLimitExceeded => DefaultAction::DumpCore,
        SignalType::FileSizeLimitExceeded => DefaultAction::DumpCore,
        SignalType::VirtualTimerExpired => DefaultAction::Terminate,
        SignalType::ProfilingTimerExpired => DefaultAction::Terminate,
        SignalType::WindowChanged => DefaultAction::Ignore,
        SignalType::IO => DefaultAction::Terminate,
        SignalType::PowerFailure => DefaultAction::Terminate,
        SignalType::BadSystemCall => DefaultAction::DumpCore,
        SignalType::ReceivedNamespace => DefaultAction::Ignore,
    }
}

fn has_stop_signals(pending: u64) -> bool {
    pending
        & ((1 << SignalType::Stop as u64)
            | (1 << SignalType::TerminalStop as u64)
            | (1 << SignalType::TTin as u64)
            | (1 << SignalType::TTou as u64))
        != 0
}
