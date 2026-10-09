// SPDX-License-Identifier: Apache-2.0
//! The panic hook: a panic is logged (scrubbed, with its thread, location and
//! backtrace) before the previous hook runs and the thread unwinds (docs/14 §2.1).
use std::any::Any;
use std::backtrace::Backtrace;

use super::scrub::clip;

/// Longest panic message kept, in characters.
const MAX_MESSAGE: usize = 1_000;
/// Longest backtrace kept, in characters.
const MAX_BACKTRACE: usize = 8_000;

/// Log every panic, then keep the previous hook's behaviour.
pub(crate) fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|at| format!("{}:{}", at.file(), at.line()))
            .unwrap_or_default();
        let backtrace = Backtrace::force_capture().to_string();
        log_panic(&location, &payload_text(info.payload()), &backtrace);
        previous(info);
    }));
}

/// Log one panic. Free text is clipped here and scrubbed by the log writer.
pub(super) fn log_panic(location: &str, payload: &str, backtrace: &str) {
    let thread = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_owned();
    let message = clip(payload, MAX_MESSAGE);
    let backtrace = clip(backtrace, MAX_BACKTRACE);
    tracing::error!(
        target: "panic",
        location,
        %thread,
        panic_message = %message,
        %backtrace,
        "panic"
    );
}

/// The text of a panic payload; non-text payloads are described, not printed.
fn payload_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "non-text panic payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_payloads_are_read_and_others_are_described() {
        assert_eq!(payload_text(&"static message"), "static message");
        assert_eq!(payload_text(&String::from("owned")), "owned");
        assert_eq!(payload_text(&42_u8), "non-text panic payload");
    }
}
