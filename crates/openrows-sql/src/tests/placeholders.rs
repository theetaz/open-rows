// SPDX-License-Identifier: Apache-2.0

use super::PG;
use crate::{PlaceholderKind, placeholders};

use PlaceholderKind::{Anonymous as A, Named as N, Positional as P};

/// Placeholder kinds and texts found in `source`.
fn found(source: &str) -> Vec<(PlaceholderKind, &str)> {
    placeholders(source, PG)
        .into_iter()
        .map(|placeholder| {
            (
                placeholder.kind,
                source.get(placeholder.range).unwrap_or("<bad range>"),
            )
        })
        .collect()
}

#[test]
fn detects_each_style() {
    let cases: &[(&str, &[(PlaceholderKind, &str)])] = &[
        (
            "SELECT * FROM t WHERE id = $1 AND v > $12",
            &[(P, "$1"), (P, "$12")],
        ),
        ("SELECT :name, :other_1", &[(N, ":name"), (N, ":other_1")]),
        (
            "SELECT * FROM t WHERE a = ? AND b = ?",
            &[(A, "?"), (A, "?")],
        ),
        ("SELECT f(:a,$2,?)", &[(N, ":a"), (P, "$2"), (A, "?")]),
        ("SELECT :ünï", &[(N, ":ünï")]),
    ];
    for (source, expected) in cases {
        assert_eq!(&found(source), expected, "source: {source:?}");
    }
}

#[test]
fn ignores_lookalikes() {
    let cases: &[&str] = &[
        "SELECT '$1 :name ?'",
        "SELECT \"$1 :name ?\"",
        "SELECT $$ $1 :name ? $$",
        "SELECT 1 -- $1 :name ?",
        "SELECT /* $1 :name ? */ 1",
        "SELECT E'\\' $1 :x ?'",
        "SELECT x::int, y::text",
        "SELECT a[1:2], a[lo:hi]",
        "SELECT :'var', :\"var\"",
        "SELECT data ?| array['a'], data ?& array['b']",
        "SELECT a$1 FROM t",
        "SELECT : name",
    ];
    for source in cases {
        assert_eq!(found(source), Vec::new(), "source: {source:?}");
    }
}

#[test]
fn reports_byte_ranges() {
    let source = "SELECT 'é', $1";
    let found = placeholders(source, PG);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].range, 13..15);
}
