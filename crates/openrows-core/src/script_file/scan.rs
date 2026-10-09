// SPDX-License-Identifier: Apache-2.0
//! The safety check of a script file before it runs: every statement is
//! classified under the connection's policy without executing anything.
use super::reader::{ScriptFileError, ScriptReader};
use crate::safety::{Gate, SafetyPolicy, Verdict, verdict};
use openrows_sql::classify;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

/// Statements shown in a confirmation, at most.
const SAMPLES: usize = 5;
/// Characters of a statement preview.
const PREVIEW: usize = 120;

/// Size and modification time: a changed file needs a new check.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileStamp {
    /// File size in bytes.
    pub bytes: u64,
    /// Last modification, when the file system records it.
    pub modified: Option<SystemTime>,
}

impl FileStamp {
    /// The stamp of the file at `path` now.
    ///
    /// # Errors
    /// The file's metadata cannot be read.
    pub fn of(path: &Path) -> Result<Self, ScriptFileError> {
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

/// A statement that needs confirmation or is blocked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlaggedStatement {
    /// 1-based line where it starts.
    pub line: usize,
    /// Its first characters on one line.
    pub preview: String,
    /// The gate's decision.
    pub verdict: Verdict,
}

/// What a script file holds, judged under a policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileScan {
    /// The file when it was checked.
    pub stamp: FileStamp,
    /// Statements in the file.
    pub statements: u64,
    /// Statements that need confirmation.
    pub confirm: u64,
    /// Statements the connection refuses.
    pub blocked: u64,
    /// The first flagged statements, blocked ones first.
    pub samples: Vec<FlaggedStatement>,
}

impl FileScan {
    /// The strictest verdict across the file.
    #[must_use]
    pub fn gate(&self) -> Gate {
        if self.blocked > 0 {
            Gate::Block
        } else if self.confirm > 0 {
            Gate::Confirm
        } else {
            Gate::Run
        }
    }

    /// Binds a confirmation to this file, unchanged, on `scope` (the
    /// connection and database).
    #[must_use]
    pub fn confirmation_token(&self, scope: &str, path: &Path) -> String {
        let mut hasher = DefaultHasher::new();
        (scope, path, self.stamp, self.statements, self.confirm).hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn flag(&mut self, line: usize, text: &str, verdict: Verdict) {
        match verdict {
            Verdict::Allow => return,
            Verdict::Confirm => self.confirm += 1,
            Verdict::Block => self.blocked += 1,
        }
        let flagged = FlaggedStatement {
            line,
            preview: preview(text),
            verdict,
        };
        let blocked = self
            .samples
            .iter()
            .filter(|sample| sample.verdict == Verdict::Block)
            .count();
        match verdict {
            Verdict::Block if blocked < SAMPLES => {
                self.samples.insert(blocked, flagged);
                self.samples.truncate(SAMPLES);
            }
            Verdict::Confirm if self.samples.len() < SAMPLES => self.samples.push(flagged),
            _ => {}
        }
    }
}

/// The statement's first characters with whitespace runs folded to one space.
#[must_use]
pub fn preview(text: &str) -> String {
    let mut out = String::new();
    for (index, word) in text.split_whitespace().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(word);
        if out.chars().count() > PREVIEW {
            let cut: String = out.chars().take(PREVIEW).collect();
            return format!("{cut}…");
        }
    }
    out
}

/// Classify every statement of the file at `path` under `policy`, stopping
/// early (with what was seen) once `stopped` returns true.
///
/// # Errors
/// The file cannot be read or is not UTF-8 text.
pub fn scan_file(
    path: &Path,
    policy: SafetyPolicy,
    stopped: impl Fn() -> bool,
) -> Result<FileScan, ScriptFileError> {
    let stamp = FileStamp::of(path)?;
    let mut scan = FileScan {
        stamp,
        statements: 0,
        confirm: 0,
        blocked: 0,
        samples: Vec::new(),
    };
    for statement in ScriptReader::open(path, policy.dialect)? {
        if stopped() {
            break;
        }
        let statement = statement?;
        scan.statements += 1;
        let class = classify(&statement.text, policy.dialect);
        scan.flag(statement.line, &statement.text, verdict(policy, class));
    }
    Ok(scan)
}
