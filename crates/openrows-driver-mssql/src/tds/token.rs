// SPDX-License-Identifier: Apache-2.0
//! The tokens of a server response (MS-TDS 2.2.7): result metadata, rows,
//! `DONE` counts, errors and messages, environment changes and the login
//! acknowledgement. Tokens the driver does not use are skipped.

use std::borrow::Cow;
use std::sync::Arc;

use super::cursor::{Cursor, Parsed, Short, invalid};
use super::envchange::{EnvChange, env_change};
use super::types::{Layout, SqlType, TypeInfo, type_info, value};

/// One result column.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Column {
    pub(crate) name: String,
    pub(crate) info: TypeInfo,
    /// `80` marks `timestamp`/`rowversion`.
    pub(crate) user_type: u32,
}

/// One row's values in column order; `None` is NULL.
pub(crate) type RowValues<'a> = Vec<Option<Cow<'a, [u8]>>>;

/// `DONE`, `DONEPROC` or `DONEINPROC`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Done {
    status: u16,
    rows: u64,
    /// `DONEINPROC`: the end of a statement inside a procedure or batch
    /// that more of the same request follows.
    pub(crate) in_proc: bool,
}

impl Done {
    /// More results of the same request follow.
    pub(crate) fn more(self) -> bool {
        self.status & 0x01 != 0
    }

    /// The statement failed or was stopped.
    pub(crate) fn error(self) -> bool {
        self.status & 0x02 != 0
    }

    /// The row count, when the server sent one.
    pub(crate) fn count(self) -> Option<u64> {
        (self.status & 0x10 != 0).then_some(self.rows)
    }

    /// Acknowledges an attention signal: the request was cancelled.
    pub(crate) fn attention(self) -> bool {
        self.status & 0x20 != 0
    }
}

/// An `ERROR` or `INFO` token.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Message {
    pub(crate) error: bool,
    pub(crate) number: i32,
    pub(crate) state: u8,
    pub(crate) class: u8,
    pub(crate) text: String,
    pub(crate) procedure: String,
    pub(crate) line: i32,
}

/// `LOGINACK`: the program and version that accepted the login.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LoginAck {
    pub(crate) program: String,
    pub(crate) version: (u8, u8, u16),
}

/// A parsed token; `rows` borrow from the token buffer.
#[derive(Debug)]
pub(crate) enum Token<'a> {
    /// Columns of the result that follows; `None` when the result has none.
    Metadata(Option<Arc<[Column]>>),
    Row(RowValues<'a>),
    Done(Done),
    Message(Message),
    EnvChange(EnvChange),
    LoginAck(LoginAck),
    /// A token the driver has no use for.
    Skipped,
}

/// Parse the token at the cursor; `columns` are the current result's.
pub(crate) fn token<'a>(cursor: &mut Cursor<'a>, columns: &[Column]) -> Parsed<Token<'a>> {
    let code = cursor.u8()?;
    match code {
        0x81 => metadata(cursor).map(Token::Metadata),
        0xD1 => row(cursor, columns, false).map(Token::Row),
        0xD2 => row(cursor, columns, true).map(Token::Row),
        0xFD..=0xFF => done(cursor, code == 0xFF).map(Token::Done),
        0xAA | 0xAB => message(cursor, code == 0xAA).map(Token::Message),
        0xE3 => env_change(cursor).map(Token::EnvChange),
        0xAD => login_ack(cursor).map(Token::LoginAck),
        0xAC => return_value(cursor).map(|()| Token::Skipped),
        0xAE => feature_ack(cursor).map(|()| Token::Skipped),
        0x79 | 0x78 => cursor.skip(4).map(|()| Token::Skipped),
        0xA9 | 0xA4 | 0xA5 | 0xED => {
            let len = cursor.u16()?;
            cursor.skip(usize::from(len)).map(|()| Token::Skipped)
        }
        0xE4 | 0xEE => {
            let len = usize::try_from(cursor.u32()?).unwrap_or(usize::MAX);
            cursor.skip(len).map(|()| Token::Skipped)
        }
        other => invalid(format!("unsupported TDS token 0x{other:02X}")),
    }
}

fn metadata(cursor: &mut Cursor<'_>) -> Parsed<Option<Arc<[Column]>>> {
    let count = cursor.u16()?;
    if count == u16::MAX {
        return Ok(None);
    }
    let mut columns = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let user_type = cursor.u32()?;
        cursor.u16()?;
        let info = type_info(cursor)?;
        if matches!(
            info.sql,
            SqlType::Text { .. } | SqlType::NText | SqlType::Image
        ) {
            for _ in 0..cursor.u8()? {
                cursor.us_varchar()?;
            }
        }
        let name = cursor.b_varchar()?;
        columns.push(Column {
            name,
            info,
            user_type,
        });
    }
    Ok(Some(columns.into()))
}

fn row<'a>(cursor: &mut Cursor<'a>, columns: &[Column], nbc: bool) -> Parsed<RowValues<'a>> {
    let bitmap = if nbc {
        cursor.take(columns.len().div_ceil(8))?
    } else {
        &[]
    };
    let mut values = Vec::with_capacity(columns.len());
    for (index, column) in columns.iter().enumerate() {
        let null = bitmap
            .get(index / 8)
            .is_some_and(|byte| byte & (1 << (index % 8)) != 0);
        values.push(if null {
            None
        } else {
            value(column.info.layout, cursor)?
        });
    }
    Ok(values)
}

fn done(cursor: &mut Cursor<'_>, in_proc: bool) -> Parsed<Done> {
    let status = cursor.u16()?;
    cursor.u16()?;
    let rows = cursor.u64()?;
    Ok(Done {
        status,
        rows,
        in_proc,
    })
}

fn message(cursor: &mut Cursor<'_>, error: bool) -> Parsed<Message> {
    let len = cursor.u16()?;
    let mut body = Cursor::new(cursor.take(usize::from(len))?);
    let parsed = (|| {
        Ok(Message {
            error,
            number: body.i32()?,
            state: body.u8()?,
            class: body.u8()?,
            text: body.us_varchar()?,
            procedure: {
                body.b_varchar()?;
                body.b_varchar()?
            },
            line: body.i32()?,
        })
    })();
    whole(parsed, "message")
}

fn login_ack(cursor: &mut Cursor<'_>) -> Parsed<LoginAck> {
    let len = cursor.u16()?;
    let mut body = Cursor::new(cursor.take(usize::from(len))?);
    let parsed = (|| {
        body.skip(5)?;
        let program = body.b_varchar()?;
        let (major, minor) = (body.u8()?, body.u8()?);
        let build = u16::from_be_bytes([body.u8()?, body.u8()?]);
        Ok(LoginAck {
            program,
            version: (major, minor, build),
        })
    })();
    whole(parsed, "LOGINACK")
}

/// A token whose length prefix was complete cannot be incomplete inside.
pub(crate) fn whole<T>(parsed: Parsed<T>, what: &str) -> Parsed<T> {
    match parsed {
        Err(Short::Incomplete) => invalid(format!("malformed {what} token")),
        other => other,
    }
}

/// `RETURNVALUE` of an output parameter; the driver binds none, so it is
/// only skipped.
fn return_value(cursor: &mut Cursor<'_>) -> Parsed<()> {
    cursor.u16()?;
    cursor.b_varchar()?;
    cursor.u8()?;
    cursor.u32()?;
    cursor.u16()?;
    let info = type_info(cursor)?;
    if info.layout == Layout::TextPtr {
        return invalid("text output parameter");
    }
    value(info.layout, cursor).map(drop)
}

/// `FEATUREEXTACK`: features the server acknowledged; none are requested.
fn feature_ack(cursor: &mut Cursor<'_>) -> Parsed<()> {
    while cursor.u8()? != 0xFF {
        let len = usize::try_from(cursor.u32()?).unwrap_or(usize::MAX);
        cursor.skip(len)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
