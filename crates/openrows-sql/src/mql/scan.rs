// SPDX-License-Identifier: Apache-2.0
use super::ParseError;
use serde_json::Value;

pub(super) fn without_comments(source: &str) -> Result<String, ParseError> {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if !quoted(ch, &mut quote, &mut escaped) && ch == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    let _ = chars.find(|ch| matches!(*ch, '\n' | '\r' | '\u{2028}' | '\u{2029}'));
                    output.push('\n');
                    continue;
                }
                Some('*') => {
                    chars.next();
                    block_comment(&mut chars)?;
                    output.push(' ');
                    continue;
                }
                _ => {}
            }
        }
        output.push(ch);
    }
    Ok(output)
}

fn block_comment(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<(), ParseError> {
    while let Some(ch) = chars.next() {
        if ch == '*' && chars.peek() == Some(&'/') {
            chars.next();
            return Ok(());
        }
    }
    Err(ParseError("unclosed comment"))
}

pub(super) fn validate_size(source: &str) -> Result<(), ParseError> {
    if source.len() > 1024 * 1024 {
        return Err(ParseError("query exceeds 1 MiB"));
    }
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for ch in source.chars() {
        if quoted(ch, &mut quote, &mut escaped) {
            continue;
        }
        match ch {
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth > 64 {
            return Err(ParseError("query nesting exceeds 64"));
        }
    }
    Ok(())
}

pub(super) fn collection(rest: &mut &str) -> Result<String, ParseError> {
    if rest.starts_with("getCollection(") {
        let (_, args) = call(rest)?;
        let name = args
            .first()
            .filter(|_| args.len() == 1)
            .and_then(Value::as_str)
            .ok_or(ParseError("invalid collection name"))?
            .to_owned();
        *rest = rest
            .strip_prefix('.')
            .ok_or(ParseError("expected collection method"))?;
        return valid_name(name);
    }
    let (name, tail) = rest
        .split_once('.')
        .ok_or(ParseError("expected collection method"))?;
    if !name
        .chars()
        .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '$')
    {
        return Err(ParseError("use getCollection for quoted names"));
    }
    *rest = tail;
    valid_name(name.to_owned())
}

fn valid_name(name: String) -> Result<String, ParseError> {
    if name.is_empty() || name.contains('\0') {
        Err(ParseError("invalid collection name"))
    } else {
        Ok(name)
    }
}

pub(super) fn call(rest: &mut &str) -> Result<(String, Vec<Value>), ParseError> {
    let (method, tail) = rest
        .split_once('(')
        .ok_or(ParseError("expected method arguments"))?;
    if !method.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(ParseError("invalid method"));
    }
    let end = closing(tail)?;
    let args = &tail[..end];
    *rest = &tail[end + 1..];
    let args = super::constructors::expand(args)?;
    let values = super::value::arguments(&format!("[{args}]"))?;
    Ok((method.to_owned(), values))
}

pub(super) fn closing(tail: &str) -> Result<usize, ParseError> {
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0_u32;
    for (offset, ch) in tail.char_indices() {
        if quoted(ch, &mut quote, &mut escaped) {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Ok(offset),
            ')' => depth -= 1,
            _ => {}
        }
    }
    Err(ParseError("unclosed arguments"))
}

fn quoted(ch: char, quote: &mut Option<char>, escaped: &mut bool) -> bool {
    if let Some(current) = *quote {
        if *escaped {
            *escaped = false;
        } else if ch == '\\' {
            *escaped = true;
        } else if ch == current {
            *quote = None;
        }
        return true;
    }
    if matches!(ch, '\'' | '"') {
        *quote = Some(ch);
        return true;
    }
    false
}
