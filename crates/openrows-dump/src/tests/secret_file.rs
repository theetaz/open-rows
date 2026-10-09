// SPDX-License-Identifier: Apache-2.0
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[cfg(unix)]
fn mode(path: &Path) -> std::io::Result<u32> {
    use std::os::unix::fs::PermissionsExt as _;
    Ok(std::fs::metadata(path)?.permissions().mode() & 0o777)
}

#[test]
fn pgpass_escapes_colons_and_backslashes() -> TestResult {
    let file = PrivateFile::pgpass(r"pa:ss\word-synthetic")?;
    let text = std::fs::read_to_string(file.path())?;
    assert_eq!(text, "*:*:*:*:pa\\:ss\\\\word-synthetic\n");
    Ok(())
}

#[test]
fn pgpass_refuses_line_breaks() {
    assert!(matches!(
        PrivateFile::pgpass("two\nlines"),
        Err(DumpError::Invalid(_))
    ));
}

#[test]
fn mysql_option_file_quotes_the_password() -> TestResult {
    let file = PrivateFile::mysql_options("say \"hi\"\\\nnow")?;
    let text = std::fs::read_to_string(file.path())?;
    assert_eq!(text, "[client]\npassword=\"say \\\"hi\\\"\\\\\\nnow\"\n");
    Ok(())
}

#[cfg(unix)]
#[test]
fn files_are_private_and_deleted_on_drop() -> TestResult {
    let file = PrivateFile::pgpass("synthetic")?;
    let path = file.path().to_path_buf();
    assert_eq!(mode(&path)?, 0o600);
    assert!(
        path.file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("openrows-pgpass-"))
    );
    drop(file);
    assert!(!path.exists());
    let options = PrivateFile::mysql_options("synthetic")?;
    assert_eq!(mode(options.path())?, 0o600);
    Ok(())
}
