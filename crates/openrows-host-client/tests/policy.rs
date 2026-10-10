// SPDX-License-Identifier: Apache-2.0
//! Reservation and crash-window policy uses no process timing assumptions.
use openrows_host_client::{
    AGGREGATE_MEMORY, CircuitBreaker, Clock, HOST_MEMORY, HostFailure, MemoryBudget,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
#[test]
fn two_hosts_reserve_aggregate_and_large_grants_are_exclusive() {
    let memory = MemoryBudget::default();
    let first = memory.host().unwrap();
    let other_profile = MemoryBudget::default();
    let second = other_profile.host().unwrap();
    assert_eq!(memory.reserved(), AGGREGATE_MEMORY);
    assert_eq!(other_profile.reserved(), AGGREGATE_MEMORY);
    assert!(matches!(memory.host(), Err(HostFailure::MemoryLimit)));
    assert!(matches!(
        memory.large(64 * 1024 * 1024),
        Err(HostFailure::MemoryLimit)
    ));
    drop(second);
    let large = memory.large(64 * 1024 * 1024).unwrap();
    assert!(matches!(memory.large(1), Err(HostFailure::MemoryLimit)));
    assert!(matches!(memory.host(), Err(HostFailure::MemoryLimit)));
    drop(large);
    assert_eq!(memory.reserved(), HOST_MEMORY);
    drop(first);
    assert_eq!(memory.reserved(), 0);
}
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Duration {
        Duration::from_secs(self.0.load(Ordering::SeqCst))
    }
}
#[test]
fn three_exits_in_sixty_seconds_refuse_fourth_with_injectable_clock() {
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let circuit = CircuitBreaker::new(clock.clone());
    for _ in 0..3 {
        assert_eq!(circuit.check(), Ok(()));
        circuit.record_exit();
    }
    assert_eq!(circuit.check(), Err(HostFailure::Spawn));
    clock.0.store(59, Ordering::SeqCst);
    assert_eq!(circuit.check(), Err(HostFailure::Spawn));
    clock.0.store(60, Ordering::SeqCst);
    assert_eq!(circuit.check(), Ok(()));
    circuit.record_exit();
    circuit.reset();
    assert_eq!(circuit.check(), Ok(()));
}
#[test]
fn driver_host_error_is_never_network_loss_or_replayed() {
    for reason in [
        HostFailure::Spawn,
        HostFailure::Incompatible,
        HostFailure::Exited,
        HostFailure::MemoryLimit,
        HostFailure::Unresponsive,
        HostFailure::Protocol,
    ] {
        let error = openrows_driver_api::DriverError::Host(reason);
        assert!(!error.is_connection_lost());
        assert!(matches!(
            error.lost_before_commit(),
            openrows_driver_api::DriverError::Host(_)
        ));
    }
}
