// SPDX-License-Identifier: Apache-2.0
//! Guessing parse options from the start of a file: its format (extension,
//! then the first character), the delimiter that splits lines most
//! consistently, and whether the first record looks like column names.
use crate::infer::ColumnStats;
use crate::options::FileFormat;
use crate::types::TypeCategory;

/// Separators tried, in preference order on a tie.
const DELIMITERS: [u8; 4] = [b',', b'\t', b';', b'|'];
/// Records compared per candidate delimiter.
const SNIFF_RECORDS: usize = 50;

/// The format named by the extension, else suggested by the first
/// non-blank character: `[` an array, `{` one object per line.
#[must_use]
pub(crate) fn format(extension: Option<&str>, head: &str) -> FileFormat {
    let extension = extension.map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("ndjson" | "jsonl") => return FileFormat::JsonLines,
        Some("csv" | "tsv" | "tab" | "txt" | "psv") => return FileFormat::Delimited,
        _ => {}
    }
    match head.trim_start().as_bytes().first() {
        Some(b'[') => FileFormat::JsonArray,
        Some(b'{') => FileFormat::JsonLines,
        _ => FileFormat::Delimited,
    }
}

/// The candidate giving the most records with the same field count (more
/// than one), then the most fields; a comma when none splits anything.
/// The last record is ignored: the sample may cut it.
#[must_use]
pub(crate) fn delimiter(head: &str, quote: Option<u8>, extension: Option<&str>) -> u8 {
    if matches!(
        extension.map(str::to_ascii_lowercase).as_deref(),
        Some("tsv" | "tab")
    ) {
        return b'\t';
    }
    let mut best = (0, 0, b',');
    for candidate in DELIMITERS {
        let widths = widths(head, candidate, quote);
        let Some(&width) = mode(&widths) else {
            continue;
        };
        let consistent = widths.iter().filter(|&&w| w == width).count();
        if width > 1 && (consistent, width) > (best.0, best.1) {
            best = (consistent, width, candidate);
        }
    }
    best.2
}

fn widths(head: &str, delimiter: u8, quote: Option<u8>) -> Vec<usize> {
    let mut parser = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .quote(quote.unwrap_or(b'"'))
        .quoting(quote.is_some())
        .has_headers(false)
        .flexible(true)
        .from_reader(head.as_bytes());
    let mut widths: Vec<usize> = parser
        .records()
        .take(SNIFF_RECORDS + 1)
        .map_while(Result::ok)
        .map(|record| record.len())
        .collect();
    if widths.len() > 1 {
        widths.pop();
    }
    widths
}

fn mode(values: &[usize]) -> Option<&usize> {
    values
        .iter()
        .max_by_key(|value| values.iter().filter(|other| other == value).count())
}

/// True when every field of `first` is non-empty, distinct and plain text
/// (no numbers, dates or booleans), as column names are.
#[must_use]
pub(crate) fn header(first: &[String]) -> bool {
    let mut names: Vec<&str> = first.iter().map(|name| name.trim()).collect();
    let texts = names.iter().all(|name| {
        let mut stats = ColumnStats::default();
        stats.observe(Some(name));
        !name.is_empty() && stats.category() == TypeCategory::Text
    });
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    texts && names.len() == count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_follow_the_extension_then_the_content() {
        assert_eq!(format(Some("NDJSON"), "[1]"), FileFormat::JsonLines);
        assert_eq!(format(Some("csv"), "{\"a\": 1}"), FileFormat::Delimited);
        assert_eq!(format(Some("json"), "  \n[{}]"), FileFormat::JsonArray);
        assert_eq!(format(Some("json"), "{\"a\": 1}\n"), FileFormat::JsonLines);
        assert_eq!(format(None, "a,b\n1,2"), FileFormat::Delimited);
    }

    #[test]
    fn delimiters_are_chosen_by_consistent_field_counts() {
        let quote = Some(b'"');
        assert_eq!(delimiter("a,b,c\n1,2,3\n4,5,6\n", quote, None), b',');
        assert_eq!(delimiter("a;b\n\"1,5\";2\n\"3,5\";4\n", quote, None), b';');
        assert_eq!(delimiter("a\tb\tc\n1\t2\t3\n", quote, None), b'\t');
        assert_eq!(delimiter("a|b\n1|2\n3|4", quote, None), b'|');
        assert_eq!(delimiter("single\ncolumn\n", quote, None), b',');
        assert_eq!(delimiter("a,b\n1,2\n", quote, Some("tsv")), b'\t');
    }

    #[test]
    fn headers_are_distinct_non_empty_text() {
        let row = |fields: &[&str]| fields.iter().map(|&f| f.to_owned()).collect::<Vec<_>>();
        assert!(header(&row(&["id", "name", "created at"])));
        assert!(!header(&row(&["1", "Ada", "2024-01-02"])));
        assert!(!header(&row(&["id", "", "x"])));
        assert!(!header(&row(&["a", "a"])));
        assert!(!header(&row(&["id", "true"])));
    }
}
