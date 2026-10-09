// SPDX-License-Identifier: Apache-2.0
//! Which copy of each tool a job uses: the path set in Settings ▸ Tools
//! (`tools.<key>` in settings.json), else the installed copy that best
//! matches the server.
use std::collections::HashMap;
use std::path::PathBuf;

use openrows_dump::{
    DumpError, FoundTool, ServerVersion, ToolKind, choose, compatibility, discover, probe,
    search_dirs,
};
use serde_json::Value;

use super::dto::{DumpToolDto, DumpToolKindDto};
use crate::connections::{IpcError, IpcErrorKind};

/// The settings.json key of each tool's path.
fn settings_key(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::PgDump => "pgDump",
        ToolKind::PgRestore => "pgRestore",
        ToolKind::Psql => "psql",
        ToolKind::MysqlDump => "mysqldump",
        ToolKind::Mysql => "mysql",
    }
}

/// Tool paths set in Settings ▸ Tools.
#[derive(Clone, Debug, Default)]
pub(super) struct Overrides(HashMap<ToolKind, PathBuf>);

impl Overrides {
    /// The non-empty `tools.*` paths of `settings`.
    pub(super) fn from_settings(settings: Option<&Value>) -> Self {
        let mut paths = HashMap::new();
        for kind in ToolKind::ALL {
            let pointer = format!("/tools/{}", settings_key(kind));
            let path = settings
                .and_then(|value| value.pointer(&pointer))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|path| !path.is_empty());
            if let Some(path) = path {
                paths.insert(kind, PathBuf::from(path));
            }
        }
        Self(paths)
    }
}

/// A tool as a job will use it.
pub(super) struct Resolved {
    pub(super) tool: Option<FoundTool>,
    pub(super) dto: DumpToolDto,
}

impl Resolved {
    /// The tool, or why there is none.
    pub(super) fn require(self) -> Result<FoundTool, IpcError> {
        self.tool.ok_or_else(|| {
            let message = self
                .dto
                .error
                .unwrap_or_else(|| format!("{} was not found", self.dto.name));
            IpcError::new(IpcErrorKind::NotFound, message)
        })
    }
}

/// Resolve `kind`: probes every installed copy (blocking, a few
/// milliseconds per copy).
pub(super) fn resolve(
    kind: ToolKind,
    overrides: &Overrides,
    server: Option<ServerVersion>,
) -> Resolved {
    let path = std::env::var_os("PATH");
    let found = discover(kind, &search_dirs(path.as_deref()));
    let label = |tool: &FoundTool| format!("{} ({})", tool.path.display(), tool.version);
    let (tool, error, from_settings) = match overrides.0.get(&kind) {
        Some(path) => match probe(kind, path) {
            Ok(tool) => (Some(tool), None, true),
            Err(error) => (None, Some(format!("Settings ▸ Tools: {error}")), true),
        },
        None => match choose(&found, server) {
            Some(tool) => (Some(tool.clone()), None, false),
            None => (
                None,
                Some(DumpError::NotFound { tool: kind.label() }.to_string()),
                false,
            ),
        },
    };
    let others = found
        .iter()
        .filter(|other| tool.as_ref().is_none_or(|tool| tool.path != other.path))
        .map(label)
        .collect();
    let dto = DumpToolDto {
        tool: DumpToolKindDto::from(kind),
        name: kind.label().to_owned(),
        path: tool.as_ref().map(|tool| tool.path.display().to_string()),
        version: tool.as_ref().map(|tool| tool.version.to_string()),
        from_settings,
        warning: tool
            .as_ref()
            .and_then(|tool| compatibility(kind, tool.version, server)),
        error,
        others,
    };
    Resolved { tool, dto }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_paths_are_read_per_tool_and_blank_means_automatic() {
        let settings = serde_json::json!({
            "tools": { "pgDump": " /opt/pg/bin/pg_dump ", "psql": "", "mysql": 3 }
        });
        let overrides = Overrides::from_settings(Some(&settings));
        assert_eq!(
            overrides.0.get(&ToolKind::PgDump),
            Some(&PathBuf::from("/opt/pg/bin/pg_dump"))
        );
        assert_eq!(overrides.0.len(), 1);
        assert!(Overrides::from_settings(None).0.is_empty());
    }

    #[test]
    fn a_broken_settings_path_is_reported_not_replaced() {
        let settings = serde_json::json!({ "tools": { "pgRestore": "/nonexistent/pg_restore" } });
        let overrides = Overrides::from_settings(Some(&settings));
        let resolved = resolve(ToolKind::PgRestore, &overrides, None);
        assert!(resolved.dto.from_settings);
        assert!(resolved.tool.is_none());
        let message = resolved.require().err().map(|error| error.message);
        assert!(message.is_some_and(|text| text.starts_with("Settings ▸ Tools: ")));
    }
}
