#[allow(clippy::module_inception)]
pub mod smp;
pub use smp::*;
pub mod ap_startup;
mod cpu_init;
pub mod cpu_locals;

pub(super) use cpu_init::cpu_init_common;
