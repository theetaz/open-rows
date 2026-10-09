// SPDX-License-Identifier: Apache-2.0
//! Running `.sql` files too large to open in the editor (FR-IO-03).
//!
//! [`ScriptReader`] streams a file's statements without loading it;
//! [`scan_file`] classifies them under the connection's safety policy
//! before anything runs (read-only blocks, confirm-before-write asks); and
//! [`run_file`] executes them batch by batch through the script runner,
//! optionally inside one transaction.
mod reader;
mod run;
mod scan;
mod tally;

pub use reader::{ScriptFileError, ScriptReader};
pub use run::{FileOutcome, FileRun, FileSummary, run_file, supports_transaction};
pub use scan::{FileScan, FileStamp, FlaggedStatement, preview, scan_file};
pub use tally::{FileEvent, FileProgress, FileStatementError};
