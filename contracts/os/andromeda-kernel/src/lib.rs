mod ack;
pub mod contract;
mod execute;
pub mod ibc;
#[cfg(all(not(target_arch = "wasm32"), feature = "testing"))]
pub mod mock;
mod proto;
mod query;
pub mod reply;
mod signature;
mod state;
mod sudo;

#[cfg(test)]
mod testing;

#[cfg(not(target_arch = "wasm32"))]
mod interface;
#[cfg(not(target_arch = "wasm32"))]
pub use crate::interface::KernelContract;
