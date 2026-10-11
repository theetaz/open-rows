// SPDX-License-Identifier: Apache-2.0
//! Shared deterministic database context fixtures.
use openrows_ai::{AiControl, ProviderConfig, ProviderKind, sharing::*};
use openrows_sql::Dialect;
/// Test failures retain source context.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
/// Remote fixture without a credential.
pub fn provider() -> ProviderConfig {
    ProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind: ProviderKind::OpenAi,
        base_url: "https://api.openai.com/v1/".into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    }
}
/// Explicit policy with custom column redaction.
pub fn policy(level: SharingLevel) -> SharingPolicy {
    SharingPolicy {
        level,
        providers: vec!["fixture".into()],
        telemetry: true,
        rules: vec![RedactionRule::Glob("public.people.note".into())],
        ..SharingPolicy::default()
    }
}
/// Requested fixture categories at each authorised level.
pub fn fixture(level: SharingLevel) -> ContextBundle {
    let mut bundle = ContextBundle { schema: vec![SchemaObject { name: "public.people".into(),
        ddl: "CREATE TABLE public.people(id INT, email TEXT, note TEXT DEFAULT 'private') -- hidden".into() }],
        query: Some("SELECT id FROM public.people WHERE id=42 AND email='private' /* hidden */".into()), ..ContextBundle::default() };
    if level >= SharingLevel::SchemaAndStats {
        bundle.plan = vec![Plan {
            operation: PlanOperation::Scan,
            parent: None,
            relation: "public.people".into(),
            predicate: "id=42 AND email='private'".into(),
            estimated_rows: 7,
        }];
        bundle.stats = vec![Aggregate {
            query: "SELECT id FROM public.people WHERE id=42".into(),
            calls: 2,
            elapsed_ms: 8,
            rows: 7,
        }];
    }
    if level == SharingLevel::SampleRows {
        bundle.samples = vec![Sample {
            table: "public.people".into(),
            columns: vec!["id".into(), "email".into(), "note".into()],
            rows: (0..3)
                .map(|index| {
                    vec![
                        Some(index.to_string()),
                        Some("private-email".into()),
                        Some("private-row".into()),
                    ]
                })
                .collect(),
        }];
    }
    bundle
}
/// Trusted fixture identities and SQL dialect.
pub fn request<'a>(
    config: &'a ProviderConfig,
    control: &'a AiControl,
    bundle: ContextBundle,
) -> GateRequest<'a> {
    GateRequest {
        connection: "connection",
        production: false,
        provider: config,
        control,
        dialect: Dialect::Postgres,
        bundle,
    }
}
