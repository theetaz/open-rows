// SPDX-License-Identifier: Apache-2.0
//! A tool invocation as an argument vector (never a shell line) and its
//! redacted display form.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Which `--verbose` lines count as finished objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressKind {
    /// `pg_dump: dumping contents of table …`.
    PgDump,
    /// `pg_restore: creating …`, `processing data …`, `executing …`.
    PgRestore,
    /// `-- Retrieving table structure for table …`.
    MysqlDump,
    /// Scripts piped through `psql` or `mysql`: bytes only.
    Script,
}

impl ProgressKind {
    /// Whether `line` reports one more finished object.
    #[must_use]
    pub fn counts(self, line: &str) -> bool {
        match self {
            Self::PgDump => line.contains("dumping contents of table"),
            Self::PgRestore => {
                line.starts_with("pg_restore: ")
                    && [
                        "creating ",
                        "processing data",
                        "processing item",
                        "executing ",
                    ]
                    .iter()
                    .any(|marker| line.contains(marker))
            }
            Self::MysqlDump => line.starts_with("-- Retrieving table structure"),
            Self::Script => false,
        }
    }
}

/// Environment variables cleared so the user's shell settings cannot
/// redirect the connection or supply another password.
pub(crate) const PG_CLEARED_ENV: &[&str] = &[
    "PGPASSWORD",
    "PGHOSTADDR",
    "PGSERVICE",
    "PGSERVICEFILE",
    "PGSSLPASSWORD",
];
/// `MySQL` counterparts of [`PG_CLEARED_ENV`].
pub(crate) const MYSQL_CLEARED_ENV: &[&str] = &["MYSQL_PWD", "MYSQL_HOST", "MYSQL_TCP_PORT"];

/// Shown instead of the temporary password file's path.
pub const PASSWORD_FILE_MARK: &str = "<password file>";

/// One tool run: program, arguments and environment, spawned directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCommand {
    /// The tool's path.
    pub program: PathBuf,
    /// Arguments, passed as they are.
    pub args: Vec<OsString>,
    /// Variables set for the tool.
    pub env: Vec<(String, OsString)>,
    /// Variables removed from the inherited environment.
    pub env_remove: Vec<&'static str>,
    /// A file piped into standard input, with byte progress.
    pub stdin: Option<PathBuf>,
    /// The file or folder the tool writes, with byte progress.
    pub output: Option<PathBuf>,
    /// How `--verbose` lines map to progress.
    pub progress: ProgressKind,
    /// The temporary password file, shown as [`PASSWORD_FILE_MARK`].
    pub password_file: Option<PathBuf>,
}

impl ToolCommand {
    /// A command for `program` with no arguments yet.
    #[must_use]
    pub fn new(program: &Path, progress: ProgressKind) -> Self {
        Self {
            program: program.to_path_buf(),
            args: Vec::new(),
            env: Vec::new(),
            env_remove: Vec::new(),
            stdin: None,
            output: None,
            progress,
            password_file: None,
        }
    }

    /// Append one argument.
    pub fn arg(&mut self, arg: impl Into<OsString>) -> &mut Self {
        self.args.push(arg.into());
        self
    }

    /// The command as a shell would show it, with the password file's
    /// path replaced by [`PASSWORD_FILE_MARK`]; the password itself is never
    /// part of a command.
    #[must_use]
    pub fn display(&self) -> String {
        let redact = |text: String| match &self.password_file {
            Some(file) => text.replace(&*file.to_string_lossy(), PASSWORD_FILE_MARK),
            None => text,
        };
        let mut words: Vec<String> = self
            .env
            .iter()
            .map(|(name, value)| format!("{name}={}", quote(&redact(lossy(value)))))
            .collect();
        words.push(quote(&lossy(self.program.as_os_str())));
        words.extend(self.args.iter().map(|arg| quote(&redact(lossy(arg)))));
        if let Some(input) = &self.stdin {
            words.push(format!("< {}", quote(&input.to_string_lossy())));
        }
        words.join(" ")
    }
}

fn lossy(value: &std::ffi::OsStr) -> String {
    value.to_string_lossy().into_owned()
}

/// Single-quote `word` for display unless it only holds safe characters;
/// the redaction mark stays readable.
fn quote(word: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "-_./=:,@+%".contains(c);
    if !word.is_empty() && (word.chars().all(safe) || word.ends_with(PASSWORD_FILE_MARK)) {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}
