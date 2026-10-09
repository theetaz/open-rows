// SPDX-License-Identifier: Apache-2.0
//! Versions printed by the tools' `--version` and reported by servers, and
//! whether a client tool can work with a server.
use std::fmt;

use crate::tools::{Family, ToolKind};

/// Who made a tool or server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    /// `PostgreSQL` (`pg_dump`, `pg_restore`, `psql`).
    Postgres,
    /// Oracle `MySQL` (`mysqldump`, `mysql`).
    Mysql,
    /// `MariaDB` (`mariadb-dump`, `mariadb`, or its `mysqldump` and `mysql`).
    Mariadb,
}

impl Product {
    /// The tool family that works with this product.
    #[must_use]
    pub fn family(self) -> Family {
        match self {
            Self::Postgres => Family::Postgres,
            Self::Mysql | Self::Mariadb => Family::Mysql,
        }
    }

    /// Display name, e.g. `PostgreSQL`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::Mariadb => "MariaDB",
        }
    }
}

/// A `major.minor.patch` release number; missing parts are zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Major release, e.g. `17` or `8`.
    pub major: u32,
    /// Minor release.
    pub minor: u32,
    /// Patch release.
    pub patch: u32,
}

impl Version {
    /// The leading release number of `text`: `17.2`, `8.4.3`, `18beta1`
    /// (18.0.0) or `11.4.2-MariaDB-log`; `None` without a leading digit.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.').map(leading_number);
        let major = parts.next().flatten()?;
        let minor = parts.next().flatten().unwrap_or(0);
        let patch = parts.next().flatten().unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// The digits a part starts with (`4-MariaDB` → 4, `18beta1` → 18).
fn leading_number(part: &str) -> Option<u32> {
    let end = part
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(part.len());
    part[..end].parse().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.patch == 0 {
            write!(formatter, "{}.{}", self.major, self.minor)
        } else {
            write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

/// What a tool's `--version` said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolVersion {
    /// The product the tool belongs to.
    pub product: Product,
    /// Its release.
    pub version: Version,
}

impl fmt::Display for ToolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.product.name(), self.version)
    }
}

/// Parse the first line of `--version` output, e.g.
/// `pg_dump (PostgreSQL) 17.2`, `mysqldump  Ver 8.4.3 for macos14 on arm64`,
/// `mysqldump  Ver 10.13 Distrib 5.7.44, for Linux` or
/// `mariadb-dump from 11.4.2-MariaDB, client 10.19 for Linux`.
#[must_use]
pub fn parse_tool_version(output: &str) -> Option<ToolVersion> {
    let line = output.lines().next()?.trim();
    if let Some(rest) = after(line, "(PostgreSQL) ") {
        return Some(ToolVersion {
            product: Product::Postgres,
            version: Version::parse(rest)?,
        });
    }
    let number = after(line, "Distrib ")
        .or_else(|| after(line, " from "))
        .or_else(|| after(line, " Ver "))?;
    let product = if line.contains("MariaDB") {
        Product::Mariadb
    } else {
        Product::Mysql
    };
    Some(ToolVersion {
        product,
        version: Version::parse(number)?,
    })
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|at| &line[at + marker.len()..])
}

/// The server a tool will talk to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerVersion {
    /// Its product.
    pub product: Product,
    /// Its release.
    pub version: Version,
}

/// Why `tool` may not work with `server`, in one sentence; `None` when it
/// should. `pg_dump` refuses servers of a newer major release, and
/// `pg_restore` cannot read archives a newer `pg_dump` wrote.
#[must_use]
pub fn compatibility(
    kind: ToolKind,
    tool: ToolVersion,
    server: Option<ServerVersion>,
) -> Option<String> {
    let server = server?;
    let (client, wanted) = (tool.version.major, server.version.major);
    if tool.product.family() != server.product.family() {
        return Some(format!(
            "{} is a {} tool, not for {}",
            kind.label(),
            tool.product.name(),
            server.product.name()
        ));
    }
    if client >= wanted {
        return (tool.product != server.product).then(|| {
            format!(
                "{} {} is the {} client; some {} features may be missed",
                kind.label(),
                tool.version,
                tool.product.name(),
                server.product.name()
            )
        });
    }
    let consequence = match kind {
        ToolKind::PgDump => "it refuses to back up newer servers",
        ToolKind::PgRestore => "it cannot read archives made by a newer pg_dump",
        _ => "some statements may not be understood",
    };
    Some(format!(
        "{} {} is older than the {} {} server: {consequence}. Install version {wanted} or newer, or choose it in Settings ▸ Tools",
        kind.label(),
        tool.version,
        server.product.name(),
        server.version,
    ))
}

#[cfg(test)]
#[path = "tests/version.rs"]
mod tests;
