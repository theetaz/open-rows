// SPDX-License-Identifier: Apache-2.0
//! Pure line parser for the supported `ssh_config` keywords.

use super::jump::{JumpSpec, parse_jump_list};

/// One parsed, typed `ssh_config` directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Directive {
    Host(Vec<String>),
    /// `Match` blocks are not supported; they never apply.
    Match,
    Include(Vec<String>),
    HostName(String),
    User(String),
    Port(u16),
    IdentityFile(String),
    IdentitiesOnly(bool),
    ProxyJump(ProxyJumpValue),
    /// A keyword outside the supported subset; ignored like `IgnoreUnknown`.
    Unsupported,
}

/// A `ProxyJump` value: explicitly disabled (`none`) or a list of hops.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProxyJumpValue {
    Disabled,
    Hops(Vec<JumpSpec>),
}

/// A directive with its 1-based line number.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Line {
    pub(crate) number: usize,
    pub(crate) directive: Directive,
}

/// A syntax error at a 1-based line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LineError {
    pub(crate) number: usize,
    pub(crate) message: String,
}

/// Parse `text` into directives; comments and blank lines are skipped.
pub(crate) fn parse_lines(text: &str) -> Result<Vec<Line>, LineError> {
    let mut lines = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let Some((keyword, rest)) = split_keyword(raw) else {
            continue;
        };
        let args = tokenize(rest).map_err(|message| LineError {
            number,
            message: message.to_owned(),
        })?;
        let directive =
            to_directive(keyword, args).map_err(|message| LineError { number, message })?;
        lines.push(Line { number, directive });
    }
    Ok(lines)
}

fn split_keyword(raw: &str) -> Option<(&str, &str)> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let (keyword, rest) = line.split_at(end);
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    Some((keyword, rest))
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Split arguments on whitespace, honouring double quotes and `#` comments.
fn tokenize(rest: &str) -> Result<Vec<String>, &'static str> {
    let mut args = Vec::new();
    let mut chars = rest.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '#' {
            break;
        } else if c == '"' {
            chars.next();
            args.push(read_quoted(&mut chars)?);
        } else {
            args.push(read_word(&mut chars));
        }
    }
    Ok(args)
}

fn read_quoted(chars: &mut Chars<'_>) -> Result<String, &'static str> {
    let mut quoted = String::new();
    for c in chars.by_ref() {
        if c == '"' {
            return Ok(quoted);
        }
        quoted.push(c);
    }
    Err("unterminated quote")
}

fn read_word(chars: &mut Chars<'_>) -> String {
    let mut word = String::new();
    while let Some(c) = chars.next_if(|c| !c.is_whitespace() && *c != '"') {
        word.push(c);
    }
    word
}

fn to_directive(keyword: &str, args: Vec<String>) -> Result<Directive, String> {
    let keyword = keyword.to_ascii_lowercase();
    match keyword.as_str() {
        "host" => non_empty(args, "Host").map(Directive::Host),
        "match" => Ok(Directive::Match),
        "include" => non_empty(args, "Include").map(Directive::Include),
        "hostname" => first(args, "HostName").map(Directive::HostName),
        "user" => first(args, "User").map(Directive::User),
        "port" => first(args, "Port")
            .and_then(|value| parse_port(&value))
            .map(Directive::Port),
        "identityfile" => first(args, "IdentityFile").map(Directive::IdentityFile),
        "identitiesonly" => first(args, "IdentitiesOnly")
            .and_then(|value| parse_yes_no(&value))
            .map(Directive::IdentitiesOnly),
        "proxyjump" => parse_proxy_jump(&first(args, "ProxyJump")?).map(Directive::ProxyJump),
        _ => Ok(Directive::Unsupported),
    }
}

fn non_empty(args: Vec<String>, keyword: &str) -> Result<Vec<String>, String> {
    if args.is_empty() {
        Err(format!("{keyword} needs an argument"))
    } else {
        Ok(args)
    }
}

fn first(args: Vec<String>, keyword: &str) -> Result<String, String> {
    args.into_iter()
        .next()
        .ok_or_else(|| format!("{keyword} needs an argument"))
}

pub(crate) fn parse_port(value: &str) -> Result<u16, String> {
    match value.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(format!("invalid port {value:?}")),
    }
}

fn parse_yes_no(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "yes" | "true" => Ok(true),
        "no" | "false" => Ok(false),
        _ => Err(format!("expected yes or no, got {value:?}")),
    }
}

fn parse_proxy_jump(value: &str) -> Result<ProxyJumpValue, String> {
    if value.eq_ignore_ascii_case("none") {
        return Ok(ProxyJumpValue::Disabled);
    }
    parse_jump_list(value).map(ProxyJumpValue::Hops)
}
