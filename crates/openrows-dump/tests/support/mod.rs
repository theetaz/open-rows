// SPDX-License-Identifier: Apache-2.0
//! Shared helpers of the dump round-trip tests: run tools inside the test
//! container with `docker exec` (the same argument vector, with the
//! password file copied to the same path inside), and read results back.
#![allow(
    dead_code,
    reason = "each engine test uses a different subset of the helpers"
)]
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use openrows_dump::{
    Canceller, FoundTool, RunEvent, RunOutcome, ToolCommand, ToolKind, parse_tool_version, run,
};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// An environment variable the test scripts export.
pub fn variable(name: &str, recipe: &str) -> TestResult<String> {
    std::env::var(name)
        .map_err(|_| format!("{name} is unset: run these ignored tests with `{recipe}`").into())
}

/// `docker <args>`; its standard output, or an error with its stderr.
pub fn docker<I, S>(args: I) -> TestResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("docker").args(args).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The container's own copy of `kind`, named `name`, with its version.
pub fn container_tool(container: &str, kind: ToolKind, name: &str) -> TestResult<FoundTool> {
    let output = docker(["exec", container, name, "--version"])?;
    let version = parse_tool_version(&output).ok_or("unparsable --version")?;
    Ok(FoundTool {
        kind,
        path: PathBuf::from(name),
        version,
    })
}

/// Copy host `file` to the same path inside `container`.
pub fn copy_in(container: &str, file: &Path) -> TestResult {
    let parent = file.parent().ok_or("file without folder")?;
    docker([
        OsString::from("exec"),
        container.into(),
        "mkdir".into(),
        "-p".into(),
        parent.into(),
    ])?;
    let mut target = OsString::from(format!("{container}:"));
    target.push(file);
    docker([OsString::from("cp"), file.into(), target])?;
    Ok(())
}

/// Copy `path` out of `container` to the host file `to`.
pub fn copy_out(container: &str, path: &str, to: &Path) -> TestResult {
    docker([
        OsString::from("cp"),
        format!("{container}:{path}").into(),
        to.into(),
    ])?;
    Ok(())
}

/// `command` run inside `container`: its password file and `files` are
/// copied in first; standard input still comes from the host.
pub fn in_container(
    command: &ToolCommand,
    container: &str,
    files: &[&Path],
) -> TestResult<ToolCommand> {
    for file in command
        .password_file
        .iter()
        .map(PathBuf::as_path)
        .chain(files.iter().copied())
    {
        copy_in(container, file)?;
    }
    let mut docker = ToolCommand::new(Path::new("docker"), command.progress);
    docker.arg("exec");
    if command.stdin.is_some() {
        docker.arg("-i");
    }
    for (name, value) in &command.env {
        let mut pair = OsString::from(format!("{name}="));
        pair.push(value);
        docker.arg("-e").arg(pair);
    }
    docker.arg(container).arg(command.program.as_os_str());
    docker.args.extend(command.args.iter().cloned());
    docker.stdin.clone_from(&command.stdin);
    Ok(docker)
}

/// The test server passwords the scripts export.
fn secrets() -> Vec<String> {
    ["OPENROWS_TEST_PG_PASSWORD", "OPENROWS_TEST_MYSQL_PASSWORD"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .filter(|secret| !secret.is_empty())
        .collect()
}

/// Fail when a test password is part of the process's argument vector or
/// environment: it may only travel in the 0600 password file.
pub fn assert_no_password(command: &ToolCommand) {
    let mut words = vec![command.program.as_os_str()];
    words.extend(command.args.iter().map(OsString::as_os_str));
    words.extend(command.env.iter().map(|(_, value)| value.as_os_str()));
    for secret in secrets() {
        for word in &words {
            assert!(
                !word.to_string_lossy().contains(&secret),
                "the password is on the command line of {}",
                command.display()
            );
        }
        assert!(!command.display().contains(&secret));
    }
}

/// Run `command` to its end; the outcome and how many objects it reported.
/// Every command a test runs is first checked for the password.
pub fn finish(command: &ToolCommand) -> TestResult<(RunOutcome, u64, Vec<String>)> {
    assert_no_password(command);
    let (mut objects, mut log) = (0, Vec::new());
    let outcome = run(command, &Canceller::default(), |event| match event {
        RunEvent::Log(lines) => log.extend(lines),
        RunEvent::Progress { objects: done, .. } => objects = done,
    })?;
    Ok((outcome, objects, log))
}

/// Run `command` and require success, showing its log otherwise.
pub fn succeed(command: &ToolCommand) -> TestResult<u64> {
    let (outcome, objects, log) = finish(command)?;
    if outcome != RunOutcome::Completed {
        return Err(format!("{outcome:?}: {}\n{}", command.display(), log.join("\n")).into());
    }
    Ok(objects)
}

/// A name unique to this test process.
pub fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", std::process::id())
}
