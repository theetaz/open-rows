// SPDX-License-Identifier: Apache-2.0
//! `.openrows/project.yaml`: the project's name, its default connection and
//! the folders (relative to the project root) that hold its `.sql` queries.
//! Plain YAML so it can be reviewed and merged in git; unknown keys are
//! ignored so later versions can add sections (such as API collections)
//! without breaking older readers.
use std::fs;
use std::io::ErrorKind;

use serde::{Deserialize, Serialize};

use super::error::ProjectError;
use super::paths::{ProjectRoot, components};

/// Folder of the manifest, relative to the project root.
pub(crate) const MANIFEST_DIR: &str = ".openrows";
/// File name of the manifest inside [`MANIFEST_DIR`].
const MANIFEST_FILE: &str = "project.yaml";
/// The manifest format this build reads and writes.
const VERSION: u32 = 1;
/// Largest manifest accepted; real ones are a few hundred bytes.
const MAX_BYTES: u64 = 64 * 1024;
/// Query folder of a new project.
const DEFAULT_FOLDER: &str = "queries";
/// Written above the YAML so a reader of the repository knows the file.
const HEADER: &str = "# OpenRows project (https://github.com/theetaz/open-rows)\n";

/// The parsed manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    #[serde(default = "version")]
    pub(crate) version: u32,
    pub(crate) name: String,
    /// Saved connection id that project queries open on by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) default_connection: Option<String>,
    /// Folders holding `.sql` queries, relative to the project root.
    #[serde(default = "default_folders")]
    pub(crate) query_folders: Vec<String>,
    /// Snippet SQL folders; older projects opt in when sharing their first snippet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) snippet_folders: Vec<String>,
    /// Relative path to an openrows.connections v1 file, never credentials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) connections_file: Option<String>,
}

const fn version() -> u32 {
    VERSION
}

fn default_folders() -> Vec<String> {
    vec![DEFAULT_FOLDER.to_owned()]
}

impl Manifest {
    /// A new project named `name` with the default query folder.
    pub(crate) fn new(name: &str) -> Self {
        Self {
            version: VERSION,
            name: name.to_owned(),
            default_connection: None,
            query_folders: default_folders(),
            snippet_folders: Vec::new(),
            connections_file: None,
        }
    }
}

/// Parse and validate manifest text.
pub(crate) fn parse(text: &str) -> Result<Manifest, ProjectError> {
    let mut manifest: Manifest =
        serde_yaml_ng::from_str(text).map_err(|error| ProjectError::Manifest(error.to_string()))?;
    if manifest.version != VERSION {
        return Err(ProjectError::Manifest(format!(
            "version {} is not supported by this OpenRows",
            manifest.version
        )));
    }
    manifest.name = manifest.name.trim().to_owned();
    if manifest.name.is_empty() {
        return Err(ProjectError::Manifest("name must not be empty".to_owned()));
    }
    manifest.query_folders = folders(&manifest.query_folders)?;
    if !manifest.snippet_folders.is_empty() {
        manifest.snippet_folders = folders(&manifest.snippet_folders)?;
    }
    if let Some(file) = &manifest.connections_file {
        components(file)?;
        if !std::path::Path::new(file)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            return Err(ProjectError::Manifest(
                "connections_file must name a .json file".into(),
            ));
        }
    }
    manifest.default_connection = manifest
        .default_connection
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty());
    Ok(manifest)
}

/// Query folders normalised to `/`-separated relative paths, deduplicated;
/// none may leave the project or point into `.openrows`.
fn folders(listed: &[String]) -> Result<Vec<String>, ProjectError> {
    let mut normal: Vec<String> = Vec::new();
    for folder in listed {
        let parts = components(folder).map_err(|_| {
            ProjectError::Manifest(format!("query folder {folder:?} leaves the project"))
        })?;
        if parts.first().is_some_and(|part| part == MANIFEST_DIR) {
            return Err(ProjectError::Manifest(format!(
                "query folder {folder:?} is inside {MANIFEST_DIR}"
            )));
        }
        let path = if parts.is_empty() {
            ".".to_owned()
        } else {
            parts.join("/")
        };
        if !normal.contains(&path) {
            normal.push(path);
        }
    }
    if normal.is_empty() {
        return Err(ProjectError::Manifest(
            "query_folders must not be empty".to_owned(),
        ));
    }
    Ok(normal)
}

/// The manifest as YAML with a header comment.
pub(crate) fn render(manifest: &Manifest) -> Result<String, ProjectError> {
    let yaml = serde_yaml_ng::to_string(manifest)
        .map_err(|error| ProjectError::Manifest(error.to_string()))?;
    Ok(format!("{HEADER}{yaml}"))
}

fn manifest_path(root: &ProjectRoot) -> Result<std::path::PathBuf, ProjectError> {
    root.resolve(&format!("{MANIFEST_DIR}/{MANIFEST_FILE}"))
}

/// Read the manifest of `root`.
pub(crate) fn load(root: &ProjectRoot) -> Result<Manifest, ProjectError> {
    let path = manifest_path(root)?;
    let missing = || ProjectError::NotProject(root.path().display().to_string());
    let meta = match fs::metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(missing()),
        Err(error) => return Err(ProjectError::io(&path, &error)),
    };
    if meta.len() > MAX_BYTES {
        return Err(ProjectError::Manifest("larger than 64 KiB".to_owned()));
    }
    let text = fs::read_to_string(&path).map_err(|error| ProjectError::io(&path, &error))?;
    parse(&text)
}

/// Write the manifest of `root`.
pub(crate) fn save(root: &ProjectRoot, manifest: &Manifest) -> Result<(), ProjectError> {
    let path = manifest_path(root)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ProjectError::io(parent, &error))?;
    }
    let text = render(manifest)?;
    crate::queries::files::write_atomic(&path, text.as_bytes()).map_err(|error| ProjectError::Io {
        path: path.display().to_string(),
        message: error.message,
    })
}

/// The manifest of `root`, creating a new project (named after the folder,
/// with an empty `queries` folder) when there is none yet.
pub(crate) fn load_or_create(root: &ProjectRoot) -> Result<Manifest, ProjectError> {
    match load(root) {
        Err(ProjectError::NotProject(_)) => {}
        other => return other,
    }
    let name = root.path().file_name().map_or_else(
        || "Project".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let manifest = Manifest::new(&name);
    let queries = root.path().join(DEFAULT_FOLDER);
    fs::create_dir_all(&queries).map_err(|error| ProjectError::io(&queries, &error))?;
    save(root, &manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_manifest_and_applies_defaults() -> Result<(), ProjectError> {
        let manifest = parse(
            "name: '  Analytics '\ndefault_connection: abc\nquery_folders: [sql, ./reports/, sql]\nfuture: {x: 1}\n",
        )?;
        assert_eq!(manifest.name, "Analytics");
        assert_eq!(manifest.default_connection.as_deref(), Some("abc"));
        assert_eq!(manifest.query_folders, vec!["sql", "reports"]);
        let minimal = parse("name: x")?;
        assert_eq!(minimal.query_folders, vec!["queries"]);
        assert_eq!((minimal.version, minimal.default_connection), (1, None));
        assert_eq!(
            parse("name: x\nquery_folders: ['.']")?.query_folders,
            vec!["."]
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_manifests() {
        for text in [
            "",
            "name: ''",
            "[1, 2]",
            "name: x\nversion: 2",
            "name: x\nquery_folders: []",
            "name: x\nquery_folders: ['../elsewhere']",
            "name: x\nquery_folders: ['/abs']",
            "name: x\nquery_folders: ['.openrows']",
        ] {
            assert!(parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn renders_yaml_that_parses_back() -> Result<(), ProjectError> {
        let mut manifest = Manifest::new("Billing");
        manifest.default_connection = Some("p1".to_owned());
        let text = render(&manifest)?;
        assert!(text.starts_with("# OpenRows project"));
        assert!(text.contains("query_folders:\n- queries\n"));
        assert_eq!(parse(&text)?, manifest);
        Ok(())
    }

    #[test]
    fn opening_a_plain_folder_creates_the_project() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root = ProjectRoot::new(dir.path())?;
        assert!(matches!(load(&root), Err(ProjectError::NotProject(_))));
        let created = load_or_create(&root)?;
        assert!(dir.path().join(".openrows/project.yaml").is_file());
        assert!(dir.path().join("queries").is_dir());
        assert_eq!(load(&root)?, created);
        assert_eq!(load_or_create(&root)?, created);
        Ok(())
    }
}
