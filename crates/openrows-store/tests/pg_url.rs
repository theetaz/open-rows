// SPDX-License-Identifier: Apache-2.0
//! `postgres://` URL parser table tests (FR-CONN-02). All credentials are
//! synthetic.

use openrows_store::profile::{EnvKind, EnvTag, TlsMode, UrlError, parse_postgres_url};

struct Case {
    url: &'static str,
    host: &'static str,
    port: u16,
    database: Option<&'static str>,
    user: Option<&'static str>,
    password: Option<&'static str>,
    tls: TlsMode,
}

const CASES: &[Case] = &[
    Case {
        url: "postgres://localhost",
        host: "localhost",
        port: 5432,
        database: None,
        user: None,
        password: None,
        tls: TlsMode::Prefer,
    },
    Case {
        url: "postgresql://app:pw@db.example.com:6543/orders",
        host: "db.example.com",
        port: 6543,
        database: Some("orders"),
        user: Some("app"),
        password: Some("pw"),
        tls: TlsMode::VerifyFull,
    },
    Case {
        url: "POSTGRES://u@10.0.0.5/db?sslmode=require",
        host: "10.0.0.5",
        port: 5432,
        database: Some("db"),
        user: Some("u"),
        password: None,
        tls: TlsMode::Require,
    },
    Case {
        url: "postgres://u:p%40ss%3Aw%2Fo+rd@[::1]:5433/my%20db",
        host: "::1",
        port: 5433,
        database: Some("my db"),
        user: Some("u"),
        password: Some("p@ss:w/o+rd"),
        tls: TlsMode::Prefer,
    },
    Case {
        url: "postgresql://[2001:db8::7]/x?sslmode=verify-ca",
        host: "2001:db8::7",
        port: 5432,
        database: Some("x"),
        user: None,
        password: None,
        tls: TlsMode::VerifyCa,
    },
    Case {
        url: "postgres:///postgres?host=%2Fvar%2Frun%2Fpostgresql&user=me",
        host: "/var/run/postgresql",
        port: 5432,
        database: Some("postgres"),
        user: Some("me"),
        password: None,
        tls: TlsMode::Prefer,
    },
    Case {
        url: "postgres://h/db?password=fr%26om-query&port=7000&dbname=other",
        host: "h",
        port: 7000,
        database: Some("other"),
        user: None,
        password: Some("fr&om-query"),
        tls: TlsMode::VerifyFull,
    },
    Case {
        url: "  postgres://u:a,b@h/d  ",
        host: "h",
        port: 5432,
        database: Some("d"),
        user: Some("u"),
        password: Some("a,b"),
        tls: TlsMode::VerifyFull,
    },
];

#[test]
fn valid_urls_parse_into_params_and_secret() {
    for case in CASES {
        let parsed = parse_postgres_url(case.url).expect(case.url);
        assert_eq!(parsed.params.host, case.host, "{}", case.url);
        assert_eq!(parsed.params.port, case.port, "{}", case.url);
        assert_eq!(
            parsed.params.database.as_deref(),
            case.database,
            "{}",
            case.url
        );
        assert_eq!(parsed.params.user.as_deref(), case.user, "{}", case.url);
        let password = parsed.password.as_ref().map(|p| p.expose().to_owned());
        assert_eq!(password.as_deref(), case.password, "{}", case.url);
        assert_eq!(parsed.tls_mode, case.tls, "{}", case.url);
        assert!(!parsed.params.options.contains_key("password"));
    }
}

#[test]
fn extra_query_params_are_kept() {
    let parsed = parse_postgres_url(
        "postgres://h/d?application_name=open%20rows&connect_timeout=5&options=-c%20x%3Dy",
    )
    .unwrap();
    assert_eq!(parsed.params.application_name.as_deref(), Some("open rows"));
    assert_eq!(parsed.params.options["connect_timeout"], "5");
    assert_eq!(parsed.params.options["options"], "-c x=y");
}

type ErrorCheck = fn(&UrlError) -> bool;

#[test]
fn malformed_urls_return_typed_errors() {
    let cases: &[(&str, ErrorCheck)] = &[
        ("mysql://h/d", |e| {
            matches!(e, UrlError::UnsupportedScheme { .. })
        }),
        ("not a url", |e| matches!(e, UrlError::Malformed(_))),
        ("", |e| matches!(e, UrlError::Malformed(_))),
        ("postgres://h:99999/d", |e| {
            matches!(e, UrlError::Malformed(_))
        }),
        ("postgres://h:abc/d", |e| {
            matches!(e, UrlError::Malformed(_))
        }),
        ("postgres://[::1/d", |e| matches!(e, UrlError::Malformed(_))),
        ("postgres://h:0/d", |e| {
            matches!(e, UrlError::InvalidPort { .. })
        }),
        ("postgres://h/d?port=x", |e| {
            matches!(e, UrlError::InvalidPort { .. })
        }),
        ("postgres://h/d?port=70000", |e| {
            matches!(e, UrlError::InvalidPort { .. })
        }),
        ("postgres://h1,h2/d", |e| {
            matches!(e, UrlError::MultipleHosts)
        }),
        ("postgres://h/d?sslmode=bogus", |e| {
            matches!(e, UrlError::InvalidSslMode { .. })
        }),
        ("postgres://u:%FF@h/d", |e| {
            matches!(e, UrlError::InvalidEncoding { field: "password" })
        }),
        ("postgres://h/%C3%28", |e| {
            matches!(e, UrlError::InvalidEncoding { field: "database" })
        }),
    ];
    for (url, expected) in cases {
        let error = parse_postgres_url(url).expect_err(url);
        assert!(expected(&error), "{url}: unexpected {error:?}");
    }
}

#[test]
fn errors_and_debug_never_contain_the_password() {
    let secret = "SyntheticCanary-9f2";
    let urls = [
        format!("postgres://u:{secret}@h:0/d"),
        format!("postgres://u:{secret}@h1,h2/d"),
        format!("postgres://u:{secret}@h/d?sslmode=bogus"),
        format!("postgres://u:{secret}@h:abc/d"),
    ];
    for url in &urls {
        let error = parse_postgres_url(url).unwrap_err();
        assert!(!format!("{error} {error:?}").contains(secret), "{error:?}");
    }
    let parsed = parse_postgres_url(&format!("postgres://u:{secret}@h/d")).unwrap();
    assert!(!format!("{parsed:?}").contains(secret));
}

#[test]
fn into_profile_applies_env_defaults_and_keeps_secret_apart() {
    let parsed = parse_postgres_url("postgres://u:pw@prod-db/app?sslmode=verify-ca").unwrap();
    let (profile, password) = parsed.into_profile("App prod", EnvTag::new(EnvKind::Prod));
    assert!(profile.safety.confirm_writes);
    assert!(!profile.safety.read_only);
    assert_eq!(profile.tls.mode, TlsMode::VerifyCa);
    assert!(profile.secrets.password.is_none());
    assert_eq!(password.unwrap().expose(), "pw");
}
