// SPDX-License-Identifier: Apache-2.0
//! The pinned protocol version and a bounded, deadline-limited `--version` probe.
use super::process::{Isolated, Launch};
use crate::AiError;
use std::{fmt, time::Duration};
use tokio::io::AsyncReadExt as _;

/// The codex-cli release whose generated app-server schema this bridge targets.
pub const TESTED_VERSION: &str = "0.166.0";
const TESTED: Version = Version(0, 166, 0);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_BYTES: u64 = 4096;

/// A `major.minor.patch` release; pre-release suffixes are ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version(u32, u32, u32);
impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.0, self.1, self.2)
    }
}
/// How a discovered release relates to the pinned protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Support {
    /// The pinned release.
    Tested,
    /// A later release of the same major version; unknown messages are ignored.
    Newer,
    /// Older than the pinned schema, or a different major version.
    Unsupported,
}
impl Version {
    pub(crate) fn support(self) -> Support {
        if self.0 != TESTED.0 || self < TESTED {
            Support::Unsupported
        } else if self == TESTED {
            Support::Tested
        } else {
            Support::Newer
        }
    }
}
/// Parse `codex-cli 0.166.0` (or a bare version) from the first output line.
pub(crate) fn parse(output: &str) -> Option<Version> {
    let token = output.lines().next()?.split_whitespace().last()?;
    let core = token.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(str::parse::<u32>);
    let version = Version(
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    parts.next().is_none().then_some(version)
}
/// Run `codex --version` isolated, with a five-second deadline and 4 KiB of output.
pub(crate) async fn probe(launch: &Launch) -> Result<Option<Version>, AiError> {
    let (child, stdin, stdout) = Isolated::spawn(launch.command(&["--version".into()])?)?;
    drop(stdin);
    let mut output = String::new();
    let read = tokio::time::timeout(
        PROBE_TIMEOUT,
        stdout.take(PROBE_BYTES).read_to_string(&mut output),
    )
    .await;
    child.stop().await;
    match read {
        Ok(Ok(_)) => Ok(parse(&output)),
        Ok(Err(_)) => Ok(None),
        Err(_) => Err(AiError::Timeout),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_parse_and_classify_against_the_pinned_schema() {
        assert_eq!(parse("codex-cli 0.166.0\n"), Some(TESTED));
        assert_eq!(parse("0.170.2-alpha.1"), Some(Version(0, 170, 2)));
        assert_eq!(parse("codex-cli 0.166"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("codex-cli 0.166.0.1"), None);
        assert_eq!(TESTED.support(), Support::Tested);
        assert_eq!(Version(0, 167, 0).support(), Support::Newer);
        assert_eq!(Version(0, 120, 9).support(), Support::Unsupported);
        assert_eq!(Version(1, 0, 0).support(), Support::Unsupported);
        assert_eq!(TESTED.to_string(), TESTED_VERSION);
    }
}
