// SPDX-License-Identifier: Apache-2.0
use super::*;

fn project(dir: &Path) -> Result<Project, ProjectError> {
    fs::create_dir_all(dir.join("queries/reports")).map_err(|e| ProjectError::io(dir, &e))?;
    Ok(Project {
        root: ProjectRoot::new(dir)?,
        manifest: Manifest::new("p"),
    })
}

#[test]
fn creates_renames_moves_duplicates_and_deletes() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let p = project(dir.path())?;
    assert_eq!(p.create_folder("queries", "daily")?, "queries/daily");
    assert_eq!(
        p.create_query("queries", "Top users", "SELECT 1")?,
        "queries/Top users.sql"
    );
    assert_eq!(
        p.create_query("queries", "Top users.sql", "SELECT 2")?,
        "queries/Top users 2.sql"
    );
    assert_eq!(
        p.rename("queries/Top users 2.sql", "second")?,
        "queries/second.sql"
    );
    assert_eq!(
        p.move_to("queries/second.sql", "queries/daily")?,
        "queries/daily/second.sql"
    );
    assert_eq!(
        p.duplicate("queries/daily/second.sql")?,
        "queries/daily/second copy.sql"
    );
    assert_eq!(
        p.duplicate("queries/daily/second.sql")?,
        "queries/daily/second copy 2.sql"
    );
    assert_eq!(p.duplicate("queries/daily")?, "queries/daily copy");
    assert!(
        dir.path()
            .join("queries/daily copy/second copy.sql")
            .is_file()
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("queries/daily/second.sql"))?,
        "SELECT 2"
    );
    p.delete("queries/daily")?;
    assert!(!dir.path().join("queries/daily").exists());
    p.delete("queries/Top users.sql")?;
    assert!(!dir.path().join("queries/Top users.sql").exists());
    Ok(())
}

#[test]
fn never_overwrites_or_moves_a_folder_into_itself() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let p = project(dir.path())?;
    p.create_query("queries", "a", "1")?;
    p.create_query("queries", "b", "2")?;
    assert!(matches!(
        p.rename("queries/a.sql", "b"),
        Err(ProjectError::Exists(_))
    ));
    assert!(matches!(
        p.create_folder("queries", "reports"),
        Err(ProjectError::Exists(_))
    ));
    p.create_folder("queries/reports", "inner")?;
    assert_eq!(
        p.move_to("queries/reports", "queries/reports/inner").err(),
        Some(ProjectError::IntoItself)
    );
    assert_eq!(fs::read_to_string(dir.path().join("queries/b.sql"))?, "2");
    Ok(())
}

#[test]
fn paths_outside_the_query_folders_are_refused() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let p = project(dir.path())?;
    fs::create_dir_all(dir.path().join(".openrows"))?;
    fs::write(dir.path().join("README.sql"), "x")?;
    for (item, result) in [
        ("queries", p.delete("queries")),
        ("README.sql", p.delete("README.sql")),
        (".openrows", p.delete(".openrows")),
        ("../x", p.delete("../x")),
    ] {
        assert!(result.is_err(), "{item}");
    }
    assert!(p.create_folder(".openrows", "x").is_err());
    assert!(p.move_to("queries/reports", ".").is_err());
    assert!(dir.path().join("README.sql").is_file());
    Ok(())
}
#[test]
fn overlapping_roots_cannot_be_deleted_or_renamed() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let mut p = project(dir.path())?;
    fs::create_dir(dir.path().join("snippets"))?;
    p.manifest.query_folders = vec![".".into(), "queries".into()];
    p.manifest.snippet_folders = vec!["snippets".into()];
    assert!(p.delete("snippets").is_err());
    assert!(p.rename("snippets", "moved").is_err());
    assert!(p.delete("queries").is_err());
    assert_eq!(
        p.create_query("snippets", "find", "SELECT 1")?,
        "snippets/find.sql"
    );
    p.delete("snippets/find.sql")?;
    assert!(dir.path().join("snippets").is_dir());
    Ok(())
}

#[test]
fn ancestors_of_configured_folders_are_protected() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let mut p = project(dir.path())?;
    fs::create_dir_all(dir.path().join("snippets/templates"))?;
    p.manifest.query_folders = vec![".".into(), "queries/reports".into()];
    p.manifest.snippet_folders = vec!["snippets/templates".into()];
    for ancestor in ["queries", "snippets"] {
        assert!(p.delete(ancestor).is_err());
        assert!(p.rename(ancestor, "moved").is_err());
        assert!(p.move_to(ancestor, "queries/reports").is_err());
        assert!(dir.path().join(ancestor).is_dir());
    }
    let file = p.create_query("snippets/templates", "find", "SELECT 1")?;
    p.delete(&file)?;
    Ok(())
}
