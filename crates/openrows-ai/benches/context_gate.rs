// SPDX-License-Identifier: Apache-2.0
//! Opt-in bounded schema/sample context construction, including policy read and redaction.
use criterion::Criterion;
use openrows_ai::{
    AiControl, ProviderConfig, ProviderKind,
    sharing::{
        ContextBundle, GateRequest, Sample, SchemaObject, SharingLevel, SharingPolicy, SharingStore,
    },
};
use openrows_sql::Dialect;
use std::{hint::black_box, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROWS_BENCH").as_deref() != Ok("1") {
        return Ok(());
    }
    let store = SharingStore::memory(1000)?;
    let provider = ProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind: ProviderKind::Ollama,
        base_url: "http://127.0.0.1:11434/".into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    };
    store.save(
        "connection",
        SharingPolicy {
            level: SharingLevel::SampleRows,
            providers: vec!["fixture".into()],
            ..SharingPolicy::default()
        },
        false,
    )?;
    let control = AiControl::default();
    control.set_enabled(true);
    let bundle = ContextBundle {
        schema: vec![SchemaObject {
            name: "public.people".into(),
            ddl: "CREATE TABLE public.people(id int, email text)".into(),
        }],
        query: Some("SELECT id FROM public.people WHERE email='private'".into()),
        samples: vec![Sample {
            table: "public.people".into(),
            columns: vec!["id".into(), "email".into()],
            rows: (0..20)
                .map(|id| vec![Some(id.to_string()), Some("private".into())])
                .collect(),
        }],
        ..ContextBundle::default()
    };
    let request = || GateRequest {
        connection: "connection",
        production: false,
        provider: &provider,
        control: &control,
        dialect: Dialect::Postgres,
        bundle: bundle.clone(),
    };
    store.gate(request())?;
    let mut criterion = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2))
        .configure_from_args();
    criterion.bench_function("context_gate_20_rows", |bench| {
        bench.iter(|| black_box(store.gate(black_box(request()))));
    });
    criterion.final_summary();
    Ok(())
}
