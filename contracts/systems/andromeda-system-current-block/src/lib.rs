pub mod contract;
#[cfg(all(not(target_arch = "wasm32"), feature = "testing"))]
pub mod mock;

#[cfg(test)]
pub mod testing;
