// SPDX-License-Identifier: Apache-2.0
use super::{TestResult, export, export_bytes, export_one, mixed, texts};
use crate::{CsvOptions, Delimiter, Format, LineEnding, Quote, TextEncoding};

fn csv(delimiter: Delimiter, header: bool) -> Format {
    Format::Csv(CsvOptions {
        delimiter,
        header,
        ..CsvOptions::default()
    })
}

#[test]
fn every_type_renders_in_its_canonical_text_and_nulls_are_empty() -> TestResult {
    let text = export_one(&csv(Delimiter::Comma, true), &mixed()?)?;
    assert_eq!(
        text,
        "id,big,ok,ratio,price,name,doc,raw,uid,day,at\r\n\
         -7,18446744073709551615,true,0.5,-12.34,\"it's \"\"ü\"\", ok\",\"{\"\"a\"\": [1, 2]}\",\
         \\x00ff10,550e8400-e29b-41d4-a716-446655440000,2024-02-29,\
         2024-02-29 12:34:56.123456+00:00\r\n\
         ,,,,,,,,,,\r\n"
    );
    Ok(())
}

#[test]
fn quoting_follows_rfc_4180() -> TestResult {
    let batch = texts(&[
        Some("plain"),
        Some("a,b"),
        Some("say \"hi\""),
        Some("line\nbreak"),
        Some("cr\rhere"),
        Some(""),
        None,
        Some("semi;colon"),
    ])?;
    let text = export_one(&csv(Delimiter::Comma, false), &batch)?;
    assert_eq!(
        text,
        "plain\r\n\"a,b\"\r\n\"say \"\"hi\"\"\"\r\n\"line\nbreak\"\r\n\"cr\rhere\"\r\n\"\"\r\n\r\nsemi;colon\r\n"
    );
    Ok(())
}

#[test]
fn the_delimiter_decides_what_needs_quotes() -> TestResult {
    let batch = texts(&[Some("a,b"), Some("a;b"), Some("a\tb"), Some("a|b")])?;
    let semicolon = export_one(&csv(Delimiter::Semicolon, true), &batch)?;
    assert_eq!(semicolon, "v\r\na,b\r\n\"a;b\"\r\na\tb\r\na|b\r\n");
    let tab = export_one(&csv(Delimiter::Tab, false), &batch)?;
    assert_eq!(tab, "a,b\r\na;b\r\n\"a\tb\"\r\na|b\r\n");
    let pipe = export_one(&csv(Delimiter::Pipe, false), &batch)?;
    assert_eq!(pipe, "a,b\r\na;b\r\na\tb\r\n\"a|b\"\r\n");
    Ok(())
}

#[test]
fn batches_stream_in_order_and_empty_results_keep_the_header() -> TestResult {
    let first = texts(&[Some("1"), Some("2")])?;
    let second = texts(&[Some("3")])?;
    let text = export(
        &csv(Delimiter::Comma, true),
        first.schema(),
        &[first.clone(), second, first.slice(0, 0)],
    )?;
    assert_eq!(text, "v\r\n1\r\n2\r\n3\r\n");
    let empty = export(&csv(Delimiter::Comma, true), first.schema(), &[])?;
    assert_eq!(empty, "v\r\n");
    Ok(())
}

#[test]
fn sliced_batches_read_their_own_rows() -> TestResult {
    let batch = mixed()?;
    let text = export_one(&csv(Delimiter::Comma, false), &batch.slice(1, 1))?;
    assert_eq!(text, ",,,,,,,,,,\r\n");
    Ok(())
}

fn with(options: CsvOptions) -> Format {
    Format::Csv(options)
}

#[test]
fn tsv_null_text_quote_and_line_endings_are_configurable() -> TestResult {
    let batch = texts(&[Some("a\tb"), None, Some("NULL"), Some("it's"), Some("")])?;
    let tsv = export_one(&with(CsvOptions::tsv()), &batch)?;
    assert_eq!(tsv, "v\r\n\"a\tb\"\r\n\r\nNULL\r\nit's\r\n\"\"\r\n");
    let custom = CsvOptions {
        quote: Quote::Single,
        null_text: "NULL".to_owned(),
        line_ending: LineEnding::Lf,
        header: false,
        ..CsvOptions::default()
    };
    let text = export_one(&with(custom), &batch)?;
    assert_eq!(text, "a\tb\nNULL\n'NULL'\n'it''s'\n''\n");
    Ok(())
}

#[test]
fn encodings_write_their_byte_order_marks() -> TestResult {
    let batch = texts(&[Some("é")])?;
    let encoded = |encoding| -> TestResult<Vec<u8>> {
        let options = CsvOptions {
            encoding,
            line_ending: LineEnding::Lf,
            ..CsvOptions::default()
        };
        export_bytes(&with(options), &batch)
    };
    assert_eq!(encoded(TextEncoding::Utf8)?, "v\né\n".as_bytes());
    assert_eq!(
        encoded(TextEncoding::Utf8Bom)?,
        b"\xEF\xBB\xBFv\n\xC3\xA9\n"
    );
    assert_eq!(encoded(TextEncoding::Latin1)?, b"v\n\xE9\n");
    assert_eq!(
        encoded(TextEncoding::Utf16Le)?,
        b"\xFF\xFEv\x00\n\x00\xE9\x00\n\x00"
    );
    let emoji = texts(&[Some("😀")])?;
    let options = CsvOptions {
        encoding: TextEncoding::Latin1,
        ..CsvOptions::default()
    };
    let error = export_bytes(&with(options), &emoji)
        .err()
        .ok_or("Latin-1 accepted an emoji")?;
    assert!(error.to_string().contains("Latin-1"), "{error}");
    Ok(())
}
