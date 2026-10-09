// SPDX-License-Identifier: Apache-2.0
//! Validation and single-pass substitution of literal command templates.
use super::HelperError;

const PLACEHOLDERS: [&str; 3] = ["{local_port}", "{remote_host}", "{remote_port}"];

pub(super) fn validate(
    program: &str,
    args: &[String],
    timeout: Option<u16>,
) -> Result<(), HelperError> {
    if !std::path::Path::new(program).is_absolute() || program.contains('\0') {
        return Err(invalid(
            "Custom command program must be an absolute path without NUL",
        ));
    }
    if !args.iter().any(|arg| arg.contains("{local_port}")) {
        return Err(invalid(
            "Custom command requires {local_port} in an argument to tell the helper where to listen",
        ));
    }
    if !matches!(timeout, None | Some(1..=300)) {
        return Err(invalid(
            "Custom command readiness timeout must be 1–300 seconds",
        ));
    }
    for arg in args {
        if arg.contains('\0') {
            return Err(invalid("Custom command arguments must not contain NUL"));
        }
        let mut rest = arg.as_str();
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else {
                return Err(invalid("Unclosed custom command placeholder"));
            };
            if !PLACEHOLDERS.contains(&&rest[start..=start + end]) {
                return Err(invalid(
                    "Unknown custom command placeholder; use {local_port}, {remote_host} or {remote_port}",
                ));
            }
            rest = &rest[start + end + 1..];
        }
    }
    Ok(())
}

pub(super) fn substitute(args: &[String], local: u16, host: &str, remote: u16) -> Vec<String> {
    let local = local.to_string();
    let remote = remote.to_string();
    args.iter()
        .map(|arg| {
            let mut output = String::with_capacity(arg.len());
            let mut rest = arg.as_str();
            while let Some(start) = rest.find('{') {
                output.push_str(&rest[..start]);
                let Some((placeholder, value)) = PLACEHOLDERS
                    .iter()
                    .zip([local.as_str(), host, remote.as_str()])
                    .find(|(placeholder, _)| rest[start..].starts_with(**placeholder))
                else {
                    break;
                };
                output.push_str(value);
                rest = &rest[start + placeholder.len()..];
            }
            output.push_str(rest);
            output
        })
        .collect()
}

fn invalid(message: &str) -> HelperError {
    HelperError::Invalid(message.into())
}
