// SPDX-License-Identifier: Apache-2.0
#![cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use super::*;
use crate::command::ProgressKind;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A shell script standing in for a tool.
fn script(dir: &Path, body: &str) -> std::io::Result<PathBuf> {
    let path = dir.join("tool");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

fn collect(
    command: &ToolCommand,
    cancel: &Canceller,
) -> Result<(RunOutcome, Vec<String>, u64, u64), DumpError> {
    let (mut lines, mut objects, mut bytes) = (Vec::new(), 0, 0);
    let outcome = run(command, cancel, |event| match event {
        RunEvent::Log(batch) => lines.extend(batch),
        RunEvent::Progress {
            objects: done,
            bytes: size,
        } => {
            objects = done;
            bytes = size;
        }
    })?;
    Ok((outcome, lines, objects, bytes))
}

#[test]
fn streams_stderr_counts_objects_and_measures_the_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("out.dump");
    let tool = script(
        dir.path(),
        "echo 'pg_dump: reading schemas' >&2\n\
         echo 'pg_dump: dumping contents of table \"public.a\"' >&2\n\
         echo 'pg_dump: dumping contents of table \"public.b\"' >&2\n\
         printf 'twelve bytes' > \"$1\"",
    )?;
    let mut command = ToolCommand::new(&tool, ProgressKind::PgDump);
    command.arg(&output);
    command.output = Some(output.clone());
    let (outcome, lines, objects, bytes) = collect(&command, &Canceller::default())?;
    assert_eq!(outcome, RunOutcome::Completed);
    assert_eq!(lines.len(), 3);
    assert_eq!((objects, bytes), (2, 12));
    Ok(())
}

#[test]
fn pipes_the_input_file_and_reports_failures_with_the_last_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("in.sql");
    std::fs::write(&input, "SELECT 1;\n".repeat(1000))?;
    let copy = dir.path().join("copy.sql");
    let tool = script(
        dir.path(),
        &format!(
            "cat > '{}'\necho 'psql:<stdin>:3: ERROR:  relation \"x\" does not exist' >&2\nexit 3",
            copy.display()
        ),
    )?;
    let mut command = ToolCommand::new(&tool, ProgressKind::Script);
    command.stdin = Some(input.clone());
    let (outcome, _, _, bytes) = collect(&command, &Canceller::default())?;
    assert_eq!(
        outcome,
        RunOutcome::Failed {
            code: Some(3),
            message: "psql:<stdin>:3: ERROR:  relation \"x\" does not exist".to_owned()
        }
    );
    assert_eq!(bytes, 10_000);
    assert_eq!(std::fs::read(&copy)?, std::fs::read(&input)?);
    Ok(())
}

#[test]
fn passes_environment_and_clears_inherited_passwords() -> TestResult {
    let dir = tempfile::tempdir()?;
    let tool = script(
        dir.path(),
        "echo \"file=$PGPASSFILE pw=${PGPASSWORD:-none}\" >&2",
    )?;
    let mut command = ToolCommand::new(&tool, ProgressKind::Script);
    command.env.push(("PGPASSFILE".to_owned(), "/x".into()));
    command.env_remove = vec!["PGPASSWORD", "HOME"];
    let (_, lines, _, _) = collect(&command, &Canceller::default())?;
    assert_eq!(lines, ["file=/x pw=none"]);
    Ok(())
}

#[test]
fn cancel_stops_the_whole_process_group() -> TestResult {
    let dir = tempfile::tempdir()?;
    // A child that would outlive its parent unless the group is signalled.
    let tool = script(
        dir.path(),
        "echo started >&2\nexec 3<&0\n(cat <&3 >/dev/null) &\nwait",
    )?;
    let mut command = ToolCommand::new(&tool, ProgressKind::Script);
    command.stdin = Some(PathBuf::from("/dev/zero"));
    let cancel = Canceller::default();
    let trigger = cancel.clone();
    let outcome = run(&command, &cancel, move |event| {
        if matches!(event, RunEvent::Log(_)) {
            trigger.cancel();
        }
    })?;
    assert_eq!(outcome, RunOutcome::Cancelled);
    Ok(())
}

#[test]
fn missing_programs_and_inputs_are_typed_errors() {
    let command = ToolCommand::new(Path::new("/nonexistent/pg_dump"), ProgressKind::PgDump);
    assert!(matches!(
        run(&command, &Canceller::default(), |_| {}),
        Err(DumpError::Io { .. })
    ));
    let mut piped = ToolCommand::new(Path::new("/bin/cat"), ProgressKind::Script);
    piped.stdin = Some(PathBuf::from("/nonexistent/in.sql"));
    assert!(run(&piped, &Canceller::default(), |_| {}).is_err());
}

#[test]
fn capture_returns_stdout_or_the_last_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let ok = script(dir.path(), "echo '1; 0 0 TABLE public t x'")?;
    let listed = capture(&ToolCommand::new(&ok, ProgressKind::Script))?;
    assert_eq!(listed, "1; 0 0 TABLE public t x\n");
    let failing = script(
        dir.path(),
        "echo 'pg_restore: error: input file does not appear to be a valid archive' >&2\nexit 1",
    )?;
    let error = capture(&ToolCommand::new(&failing, ProgressKind::Script)).err();
    assert!(error.is_some_and(|error| error.to_string().contains("valid archive")));
    Ok(())
}

#[test]
fn folder_sizes_add_up_their_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("a"), [0; 10])?;
    std::fs::write(dir.path().join("b"), [0; 5])?;
    assert_eq!(size_of(dir.path()), 15);
    assert_eq!(size_of(&dir.path().join("missing")), 0);
    Ok(())
}
