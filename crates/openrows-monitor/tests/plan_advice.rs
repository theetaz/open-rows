// SPDX-License-Identifier: Apache-2.0
//! Plan projection resolves catalog identities without retaining predicate values.
use openrows_explain::{Format, analyse, parse};
use openrows_monitor::advisor::{AdvisorInput, Coverage, TableEvidence, advise, project_plan};

#[test]
fn projection_redacts_conditions_and_resolves_only_simple_keys()
-> Result<(), Box<dyn std::error::Error>> {
    let input = AdvisorInput {
        now_ms: 0.0,
        tables_coverage: Coverage {
            collected_ms: 0.0,
            complete: true,
        },
        indexes_coverage: Coverage {
            collected_ms: 0.0,
            complete: true,
        },
        tables: vec![TableEvidence {
            id: "t".into(),
            schema: "public".into(),
            name: "events".into(),
            estimated_rows: Some(200_000.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    for (filter, candidate) in [
        ("(key = 'test-only'::text)", true),
        ("(lower(key) = 'test-only'::text)", false),
        ("((key = 1) OR (other = 2))", false),
    ] {
        let raw = serde_json::json!([{ "Plan": { "Node Type": "Seq Scan", "Schema": "public", "Relation Name": "events", "Plan Rows": 10, "Filter": filter } }]).to_string();
        let plan = analyse(parse(Format::PostgresJson, &[vec![Some(raw)]])?);
        let projected = project_plan(&plan, ("q", 0.0), &input.tables);
        assert!(!format!("{projected:?}").contains("test-only"));
        let mut evidence = input.clone();
        evidence.plans.push(projected);
        assert_eq!(
            advise(&evidence)
                .iter()
                .any(|c| c.rule_id == "pg.selective-scan"),
            candidate
        );
    }
    Ok(())
}

#[test]
fn equality_join_projection_resolves_alias_key_without_literals()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = serde_json::json!([{ "Plan": { "Node Type": "Hash Join", "Hash Cond": "(e.key = o.key)", "Plans": [
        {"Node Type": "Seq Scan", "Relation Name": "events", "Alias": "e", "Plan Rows": 200_000},
        {"Node Type": "Seq Scan", "Relation Name": "other", "Alias": "o", "Plan Rows": 1}
    ]}}]).to_string();
    let parsed = analyse(parse(Format::PostgresJson, &[vec![Some(raw)]])?);
    let table = TableEvidence {
        id: "t".into(),
        schema: "public".into(),
        name: "events".into(),
        estimated_rows: Some(200_000.0),
        ..Default::default()
    };
    let plan = project_plan(&parsed, ("q", 0.0), std::slice::from_ref(&table));
    assert_eq!(plan.nodes[1].join_columns, vec!["key"]);
    let input = AdvisorInput {
        now_ms: 0.0,
        tables_coverage: Coverage {
            complete: true,
            collected_ms: 0.0,
        },
        indexes_coverage: Coverage {
            complete: true,
            collected_ms: 0.0,
        },
        tables: vec![table],
        plans: vec![plan],
        ..Default::default()
    };
    assert!(
        advise(&input)
            .iter()
            .any(|card| card.rule_id == "pg.join-key"
                && card.sql.as_deref()
                    == Some("CREATE INDEX CONCURRENTLY ON \"public\".\"events\" (\"key\");"))
    );
    Ok(())
}

fn catalog_reading(columns: &[&str], values: &[&str]) -> openrows_monitor::Reading {
    openrows_monitor::Reading {
        terminal: false,
        state: openrows_monitor::SampleState::Ok,
        epoch: "e".into(),
        partial: false,
        columns: columns
            .iter()
            .map(|name| openrows_monitor::Column {
                name: (*name).into(),
                unit: "metadata".into(),
                kind: openrows_monitor::MetricKind::Text,
            })
            .collect(),
        rows: vec![values.iter().map(|value| Some((*value).into())).collect()],
    }
}

#[test]
fn malformed_index_metadata_cannot_establish_index_absence() {
    use openrows_monitor::advisor::{NodeEvidence, PlanEvidence};
    use openrows_monitor::{LiveStore, Source};
    let mut store = LiveStore::default();
    store.ingest(
        Source::Tables,
        catalog_reading(
            &[
                "relation_id",
                "schema",
                "table",
                "estimated_rows",
                "size_bytes",
            ],
            &["t", "public", "events", "200000", "8000000"],
        ),
        0,
        0.0,
    );
    let reading = catalog_reading(
        &[
            "index_id",
            "relation_id",
            "schema",
            "index",
            "key_columns",
            "definition_key",
            "usable",
            "protected",
        ],
        &[
            "i",
            "t",
            "public",
            "events_key",
            "[\"key\"]",
            "signature",
            "t",
            "f",
        ],
    );
    store.record_plan(PlanEvidence {
        statement_id: "q".into(),
        collected_ms: 0.0,
        measured: false,
        nodes: vec![NodeEvidence {
            table_id: Some("t".into()),
            operator: "Seq Scan".into(),
            estimated_rows: Some(1.0),
            filter_column: Some("key".into()),
            ..Default::default()
        }],
    });
    for field in [4, 5, 6] {
        let mut malformed = reading.clone();
        malformed.rows[0][field] = None;
        store.ingest(Source::Indexes, malformed, 0, 0.0);
        let input = store.advisor_input(0.0);
        assert!(!input.indexes_coverage.complete);
        assert!(
            !advise(&input)
                .iter()
                .any(|card| card.rule_id == "pg.selective-scan")
        );
    }
}
