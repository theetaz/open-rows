// SPDX-License-Identifier: Apache-2.0
//! `openrows open <target>` (FR-APP-09, docs/16 §3.1). The command hands a
//! link or file to the app through `LaunchServices` (`/usr/bin/open -a`), which
//! routes it to the running instance or starts the app, so there is always
//! one instance per bundle. The command never opens a window or connects
//! itself; the app shows the link's confirmation as for any other link.
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use url::form_urlencoded::Serializer;

use crate::links::{self, LinkError};

/// What the command hands to the app.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    Link(String),
    File(PathBuf),
}

/// Why the command could not hand over its target.
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(
        "usage: openrows open <openrows:// link | file | connection <name or id>>\n\
         Links and files open in the running app (it starts when needed); \
         nothing connects before you confirm it there."
    )]
    Usage,
    #[error("{0}")]
    Link(#[from] LinkError),
    #[error("the file does not exist or cannot be read")]
    File,
    #[error("run this command from inside OpenRows.app (Contents/MacOS)")]
    NoBundle,
    #[cfg(not(target_os = "macos"))]
    #[error("opening links from the command line is supported on macOS only")]
    Unsupported,
    #[error("could not reach OpenRows: {0}")]
    Launch(#[from] std::io::Error),
}

impl CliError {
    fn code(&self) -> u8 {
        if matches!(self, Self::Launch(_)) {
            1
        } else {
            2
        }
    }
}

/// Run `open …` when those are the arguments (without the program name);
/// `None` lets the app start normally.
pub fn open_command(args: impl IntoIterator<Item = OsString>) -> Option<ExitCode> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.first().map(OsString::as_os_str) != Some(OsStr::new("open")) {
        return None;
    }
    Some(match run(&args[1..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error);
            ExitCode::from(error.code())
        }
    })
}

#[expect(
    clippy::print_stderr,
    reason = "the command line reports its own usage errors"
)]
fn report(error: &CliError) {
    eprintln!("openrows: {error}");
}

fn run(args: &[OsString]) -> Result<(), CliError> {
    let target = target(args, &std::env::current_dir()?)?;
    let executable = std::env::current_exe()?.canonicalize()?;
    let bundle = bundle_of(&executable).ok_or(CliError::NoBundle)?;
    launch(&bundle, &target)
}

fn target(args: &[OsString], cwd: &Path) -> Result<Target, CliError> {
    let text = |arg: &OsString| arg.to_str().map(str::to_owned).ok_or(CliError::Usage);
    match args {
        [kind, reference] if kind == "connection" => {
            let link = connection_link(&text(reference)?);
            links::parse(&link)?;
            Ok(Target::Link(link))
        }
        [one]
            if one
                .to_string_lossy()
                .starts_with(&format!("{}:", links::SCHEME)) =>
        {
            let link = text(one)?;
            links::parse(&link)?;
            Ok(Target::Link(link))
        }
        [one] if !one.to_string_lossy().starts_with('-') => cwd
            .join(one)
            .canonicalize()
            .map(Target::File)
            .map_err(|_| CliError::File),
        _ => Err(CliError::Usage),
    }
}

/// A connection link naming the profile by id when `reference` is one, else by name.
fn connection_link(reference: &str) -> String {
    let key = if crate::connections::profiles::profile_id(reference).is_ok() {
        "id"
    } else {
        "name"
    };
    let query = Serializer::new(String::new())
        .append_pair(key, reference)
        .finish();
    format!("{}://connection?{query}", links::SCHEME)
}

/// `X.app` for an executable at `X.app/Contents/MacOS/<binary>`.
fn bundle_of(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let named = |path: &Path, name: &str| path.file_name() == Some(OsStr::new(name));
    let is_app = bundle.extension() == Some(OsStr::new("app"));
    (named(macos, "MacOS") && named(contents, "Contents") && is_app).then(|| bundle.to_owned())
}

#[cfg(target_os = "macos")]
fn launch(bundle: &Path, target: &Target) -> Result<(), CliError> {
    let argument = match target {
        Target::Link(link) => OsStr::new(link),
        Target::File(path) => path.as_os_str(),
    };
    let status = std::process::Command::new("/usr/bin/open")
        .arg("-a")
        .arg(bundle)
        .arg(argument)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("LaunchServices refused the request").into())
    }
}

#[cfg(not(target_os = "macos"))]
fn launch(_: &Path, _: &Target) -> Result<(), CliError> {
    Err(CliError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_the_open_command_is_handled() {
        assert_eq!(open_command(args(&[])), None);
        assert_eq!(open_command(args(&["-psn_0_1"])), None);
        assert_eq!(open_command(args(&["open"])), Some(ExitCode::from(2)));
    }

    #[test]
    fn targets_are_validated_before_launch() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("q.sql"), "SELECT 1")?;
        let cwd = dir.path();
        assert_eq!(
            target(&args(&["q.sql"]), cwd)?,
            Target::File(cwd.join("q.sql").canonicalize()?)
        );
        assert!(matches!(
            target(&args(&["missing.sql"]), cwd),
            Err(CliError::File)
        ));
        assert!(matches!(
            target(&args(&["openrows://connection?name=a&password=b"]), cwd),
            Err(CliError::Link(LinkError::Secret))
        ));
        assert!(matches!(
            target(&args(&["--help"]), cwd),
            Err(CliError::Usage)
        ));
        assert_eq!(
            target(&args(&["connection", "Local PG"]), cwd)?,
            Target::Link("openrows://connection?name=Local+PG".to_owned())
        );
        let id = "0192d3c4-5b6a-7c8d-9e0f-a1b2c3d4e5f6";
        assert_eq!(
            target(&args(&["connection", id]), cwd)?,
            Target::Link(format!("openrows://connection?id={id}"))
        );
        Ok(())
    }

    #[test]
    fn only_bundled_executables_can_forward() {
        let inside = Path::new("/Applications/OpenRows.app/Contents/MacOS/openrows-app");
        assert_eq!(
            bundle_of(inside),
            Some(PathBuf::from("/Applications/OpenRows.app"))
        );
        assert_eq!(bundle_of(Path::new("/target/debug/openrows-app")), None);
        assert_eq!(bundle_of(Path::new("/x/Contents/MacOS/openrows-app")), None);
    }
}
