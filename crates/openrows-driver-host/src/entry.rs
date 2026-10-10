// SPDX-License-Identifier: Apache-2.0
//! Shared process entry for the production helper and separately named test fixture.

/// Run the contained host process without exposing native diagnostics.
pub fn run() {
    std::panic::set_hook(Box::new(|_| {}));
    #[cfg(unix)]
    {
        let _ = rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        );
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build();
    let success =
        runtime.is_ok_and(|runtime| runtime.block_on(openrows_driver_host::run()).is_ok());
    // Native workers may be blocked; process exit is the deliberate containment boundary.
    std::process::exit(if success { 0 } else { 70 });
}
