// SPDX-License-Identifier: Apache-2.0
use super::*;

fn row(cells: &[Option<&str>]) -> TextRow {
    cells.iter().map(|cell| cell.map(str::to_owned)).collect()
}

#[test]
fn identifier_lists_unquote_like_quote_ident() -> Result<(), DriverError> {
    assert_eq!(split_identifiers("")?, Vec::<String>::new());
    assert_eq!(split_identifiers("id")?, vec!["id"]);
    assert_eq!(
        split_identifiers(r#"id,"a,b","x""y","Upper""#)?,
        vec!["id", "a,b", "x\"y", "Upper"]
    );
    assert!(split_identifiers(r#""open"#).is_err());
    assert!(split_identifiers(r#""a"b"#).is_err());
    Ok(())
}

#[test]
fn columns_map_generated_kinds() -> Result<(), DriverError> {
    let identity = column(&row(&[
        Some("id"),
        Some("integer"),
        Some("f"),
        None,
        Some("a"),
        None,
    ]))?;
    assert_eq!(identity.generated, Generated::IdentityAlways);
    let stored = column(&row(&[
        Some("total"),
        Some("numeric"),
        Some("t"),
        Some("(price * qty)"),
        Some("s"),
        Some("Line total"),
    ]))?;
    assert_eq!(stored.generated, Generated::Stored);
    assert_eq!(stored.default.as_deref(), Some("(price * qty)"));
    let plain = column(&row(&[
        Some("n"),
        Some("text"),
        Some("t"),
        None,
        Some(""),
        None,
    ]))?;
    assert_eq!(plain.generated, Generated::No);
    assert!(
        column(&row(&[
            Some("n"),
            Some("text"),
            Some("t"),
            None,
            Some("z"),
            None
        ]))
        .is_err()
    );
    Ok(())
}

#[test]
fn constraints_map_kinds_columns_and_references() -> Result<(), DriverError> {
    let foreign = constraint(&row(&[
        Some("orders_user_fk"),
        Some("f"),
        Some("FOREIGN KEY (user_id) REFERENCES users(id)"),
        Some("user_id"),
        Some("public"),
        Some("users"),
        Some("id"),
    ]))?;
    assert_eq!(foreign.kind, ConstraintKind::Foreign);
    assert_eq!(foreign.columns, vec!["user_id"]);
    assert_eq!(
        foreign.references,
        Some(ForeignTarget {
            schema: "public".into(),
            table: "users".into(),
            columns: vec!["id".into()],
        })
    );
    let check = constraint(&row(&[
        Some("price_positive"),
        Some("c"),
        Some("CHECK (price > 0::numeric)"),
        Some(""),
        None,
        None,
        Some(""),
    ]))?;
    assert_eq!(check.kind, ConstraintKind::Check);
    assert!(check.columns.is_empty() && check.references.is_none());
    assert!(
        constraint(&row(&[
            Some("n"),
            Some("t"),
            Some("x"),
            None,
            None,
            None,
            None
        ]))
        .is_err()
    );
    Ok(())
}

#[test]
fn indexes_map_include_columns_and_constraint_flag() -> Result<(), DriverError> {
    let index = index(&row(&[
        Some("orders_lookup"),
        Some("btree"),
        Some("t"),
        Some("f"),
        Some("t"),
        Some("customer_id, lower(email)"),
        Some(r#"total,"Note""#),
        Some("deleted_at IS NULL"),
        Some("Lookup"),
        Some("CREATE UNIQUE INDEX orders_lookup ON public.orders USING btree (customer_id)"),
    ]))?;
    assert!(index.unique && !index.primary && index.constraint);
    assert_eq!(index.include, vec!["total", "Note"]);
    assert_eq!(index.predicate.as_deref(), Some("deleted_at IS NULL"));
    Ok(())
}

#[test]
fn trigger_bits_decode_timing_events_and_level() -> Result<(), DriverError> {
    // ROW | BEFORE | INSERT | UPDATE
    let before = trigger(&row(&[
        Some("touch"),
        Some("23"),
        Some("O"),
        Some("public.touch()"),
        Some("CREATE TRIGGER touch …"),
    ]))?;
    assert_eq!(before.timing, TriggerTiming::Before);
    assert_eq!(
        before.events,
        vec![TriggerEvent::Insert, TriggerEvent::Update]
    );
    assert!(before.row_level && before.enabled);
    // statement-level AFTER DELETE | TRUNCATE, disabled
    let after = trigger(&row(&[
        Some("audit"),
        Some("40"),
        Some("D"),
        Some("public.audit()"),
        Some("CREATE TRIGGER audit …"),
    ]))?;
    assert_eq!(after.timing, TriggerTiming::After);
    assert_eq!(
        after.events,
        vec![TriggerEvent::Delete, TriggerEvent::Truncate]
    );
    assert!(!after.row_level && !after.enabled);
    let instead = trigger(&row(&[
        Some("v"),
        Some("69"),
        Some("A"),
        Some("f()"),
        Some("d"),
    ]))?;
    assert_eq!(instead.timing, TriggerTiming::InsteadOf);
    assert!(
        trigger(&row(&[
            Some("x"),
            Some("-1"),
            Some("O"),
            Some("f"),
            Some("d")
        ]))
        .is_err()
    );
    Ok(())
}

#[test]
fn table_info_maps_sizes_and_estimates() -> Result<(), DriverError> {
    let info = table_info(&row(&[
        Some("r"),
        Some("1788"),
        Some("82944"),
        Some("16384"),
        Some("66560"),
        Some("Users"),
        None,
    ]))?;
    assert_eq!(info.kind, RelationKind::Table);
    assert_eq!(info.row_estimate, Some(1788));
    assert_eq!(
        (info.total_bytes, info.data_bytes, info.index_bytes),
        (82944, 16384, 66560)
    );
    assert!(
        table_info(&row(&[
            Some("r"),
            None,
            Some("-1"),
            Some("0"),
            Some("0"),
            None,
            None
        ]))
        .is_err()
    );
    assert!(
        table_info(&row(&[
            Some("i"),
            None,
            Some("0"),
            Some("0"),
            Some("0"),
            None,
            None
        ]))
        .is_err()
    );
    Ok(())
}
