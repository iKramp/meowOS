use core::any::Any;
use std::alloc::boxed::Box;

use crate::proc::{
    NamespaceHolder, Pid, ProcessData,
    signal::{SignalData, SignalFields, SignalType},
};

struct RecvNamespaceSignal {
    source: Pid,
    namespace_id: u64,
}

//proc can't receive namespaces. Remove the namespace
pub(super) fn run_default_handler(proc: &ProcessData, signal_data: SignalData) {
    if signal_data.signal_type != SignalType::ReceivedNamespace {
        debug_assert!(
            false,
            "ran recv_namespace_signal default handler for non-recv_namespace signal: {:?}",
            signal_data.signal_type
        );
        return;
    }

    let fields_any: &dyn Any = signal_data.fields.as_ref();
    let res = fields_any.downcast_ref::<RecvNamespaceSignal>();

    let Some(recv_signal) = res else {
        debug_assert!(
            false,
            "ran recv_namespace_signal default handler for signal {:?} without special data",
            signal_data.signal_type
        );
        return;
    };

    let mut proc_mut = proc.get_mutable();
    let namespaces = proc_mut.get_namespaces_mut();
    let _ = namespaces.remove_namespace(recv_signal.namespace_id);
}

pub fn generate_recv_namespace_signal(source: Pid, target: &ProcessData, namespace: NamespaceHolder) -> SignalData {
    let namespace_id = namespace.get_id();

    let mut proc_mut = target.get_mutable();
    let namespaces = proc_mut.get_namespaces_mut();
    namespaces.add_namespace(namespace);
    drop(proc_mut);

    SignalData {
        signal_type: SignalType::ReceivedNamespace,
        source,
        fields: Box::new(RecvNamespaceSignal { source, namespace_id }),
    }
}

impl SignalFields for RecvNamespaceSignal {
    fn get_data_as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        let ptr = self as *const Self as *const u8;
        unsafe { core::slice::from_raw_parts(ptr, size) }
    }
}
