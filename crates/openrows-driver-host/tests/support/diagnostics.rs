// SPDX-License-Identifier: Apache-2.0
//! Preserve the supervisor's bounded startup failure categories in test output.

/// Install startup diagnostics once in each independent test process.
pub(super) fn init() {
    static DIAGNOSTICS: std::sync::Once = std::sync::Once::new();
    DIAGNOSTICS.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(std::io::stderr)
            .try_init();
    });
}
