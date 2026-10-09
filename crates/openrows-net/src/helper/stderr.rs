// SPDX-License-Identifier: Apache-2.0
//! Bounded diagnostic output; suspicious lines are redacted in full.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncReadExt as _, BufReader};

#[derive(Clone, Default)]
pub(super) struct Stderr(Arc<Mutex<VecDeque<String>>>);
impl Stderr {
    pub(super) fn push(&self, line: &str) {
        let text = scrub(line);
        let mut lines = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if lines.len() == 200 {
            lines.pop_front();
        }
        lines.push_back(text);
    }
    pub(super) fn text(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub(super) async fn capture(&self, pipe: tokio::process::ChildStderr) {
        let mut reader = BufReader::new(pipe);
        let mut bytes = [0; 1024];
        let mut line = Line::default();
        while let Ok(count) = reader.read(&mut bytes).await {
            if count == 0 {
                break;
            }
            for byte in &bytes[..count] {
                line.consume(*byte, self);
            }
        }
        if !line.bytes.is_empty() {
            self.push(line.text());
        }
    }
}
#[derive(Default)]
struct Line {
    bytes: Vec<u8>,
    truncated: bool,
}
impl Line {
    fn consume(&mut self, byte: u8, stderr: &Stderr) {
        if byte == b'\n' {
            stderr.push(self.text());
            self.bytes.clear();
            self.truncated = false;
        } else if self.bytes.len() < 4096 {
            self.bytes.push(byte);
        } else {
            self.truncated = true;
        }
    }
    fn text(&self) -> &str {
        if self.truncated {
            "[oversized stderr line redacted]"
        } else {
            std::str::from_utf8(&self.bytes).unwrap_or("[invalid stderr]")
        }
    }
}
/// Redact complete lines containing credential-like assignments or headers.
#[must_use]
pub fn scrub(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if [
        "token=",
        "password=",
        "authorization:",
        "token =",
        "password =",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        "[credential-bearing stderr redacted]".into()
    } else {
        line.chars().take(4096).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_keeps_last_two_hundred_lines_and_scrubs_before_storage() {
        let stderr = Stderr::default();
        for index in 0..205 {
            stderr.push(&format!("line {index}"));
        }
        stderr.push("Authorization: x");
        let text = stderr.text();
        assert_eq!(text.lines().count(), 200);
        assert!(text.starts_with("line 6\n"));
        assert!(!text.contains("Authorization: x"));
        assert_eq!(scrub(&"a".repeat(5000)).len(), 4096);
    }
}
