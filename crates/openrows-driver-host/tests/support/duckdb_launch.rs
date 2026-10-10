// SPDX-License-Identifier: Apache-2.0
//! Shared native test launch without unused driver fixture APIs.
use openrows_host_client::{CircuitBreaker, HostDriverId, LaunchOptions, MemoryBudget};
use std::path::PathBuf;
/// One native fixture launch without application dependencies.
pub fn options(memory: MemoryBudget) -> LaunchOptions {
    LaunchOptions {
        driver: HostDriverId::Duckdb,
        executable: PathBuf::from(env!("CARGO_BIN_EXE_host-test-fixture")),
        endpoint_root: if cfg!(unix) {
            "/tmp".into()
        } else {
            std::env::temp_dir()
        },
        memory,
        circuit: CircuitBreaker::default(),
    }
}
