// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::tds::cursor::{Short, utf16_bytes};

fn parse<'a>(bytes: &'a [u8], columns: &[Column]) -> Parsed<Token<'a>> {
    token(&mut Cursor::new(bytes), columns)
}

fn b_varchar(text: &str) -> Vec<u8> {
    let mut out = vec![u8::try_from(text.len()).unwrap_or(0)];
    out.extend(utf16_bytes(text));
    out
}

#[test]
fn metadata_then_rows_and_null_bitmaps_decode() -> Result<(), String> {
    // One INTN(4) column `n` and one NVARCHAR(10) column `s`.
    let mut meta = vec![0x81, 2, 0];
    meta.extend([0, 0, 0, 0, 1, 0, 0x26, 4]);
    meta.extend(b_varchar("n"));
    meta.extend([0, 0, 0, 0, 1, 0, 0xE7, 20, 0, 9, 4, 0xD0, 0, 52]);
    meta.extend(b_varchar("s"));
    let Ok(Token::Metadata(Some(columns))) = parse(&meta, &[]) else {
        return Err("metadata expected".into());
    };
    assert_eq!(columns[0].name, "n");
    assert_eq!(columns[1].info.layout, Layout::UShortLen);
    let mut row = vec![0xD1, 4];
    row.extend(7_i32.to_le_bytes());
    row.extend([2, 0, b'x', 0]);
    let Ok(Token::Row(values)) = parse(&row, &columns) else {
        return Err("row expected".into());
    };
    assert_eq!(values[0].as_deref(), Some(&7_i32.to_le_bytes()[..]));
    let nbc = [0xD2, 0b10, 4, 1, 0, 0, 0];
    let Ok(Token::Row(values)) = parse(&nbc, &columns) else {
        return Err("row expected".into());
    };
    assert!(values[1].is_none());
    assert!(matches!(parse(&row[..5], &columns), Err(Short::Incomplete)));
    Ok(())
}

#[test]
fn done_messages_and_skipped_tokens_parse() -> Result<(), String> {
    let mut done = vec![0xFD, 0x31, 0, 0xC1, 0];
    done.extend(3_u64.to_le_bytes());
    let Ok(Token::Done(parsed)) = parse(&done, &[]) else {
        return Err("done expected".into());
    };
    assert!(parsed.more() && parsed.attention() && !parsed.in_proc);
    assert_eq!(parsed.count(), Some(3));
    let mut body = 50_000_i32.to_le_bytes().to_vec();
    body.extend([1, 16]);
    body.extend(u16::try_from(4).unwrap_or(0).to_le_bytes());
    body.extend(utf16_bytes("oops"));
    body.extend(b_varchar("srv"));
    body.extend(b_varchar("p"));
    body.extend(3_i32.to_le_bytes());
    let mut error = vec![0xAA];
    error.extend(u16::try_from(body.len()).unwrap_or(0).to_le_bytes());
    error.extend(&body);
    let Ok(Token::Message(message)) = parse(&error, &[]) else {
        return Err("message expected".into());
    };
    assert_eq!(
        (
            message.error,
            message.number,
            message.text.as_str(),
            message.line
        ),
        (true, 50_000, "oops", 3)
    );
    assert!(matches!(
        parse(&[0x79, 0, 0, 0, 0], &[]),
        Ok(Token::Skipped)
    ));
    assert!(matches!(
        parse(&[0xA9, 2, 0, 1, 0], &[]),
        Ok(Token::Skipped)
    ));
    assert!(matches!(parse(&[0x42], &[]), Err(Short::Invalid(_))));
    Ok(())
}
