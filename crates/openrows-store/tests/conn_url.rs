// SPDX-License-Identifier: Apache-2.0
//! Connection URL parser table tests (FR-CONN-02). All credentials are
//! synthetic.

use openrows_store::profile::{Driver, EnvKind, EnvTag, TlsMode, UrlError, parse_url};

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
        let parsed = parse_url(case.url).expect(case.url);
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
    let parsed = parse_url(
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
        ("redis://h/d", |e| {
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
        let error = parse_url(url).expect_err(url);
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
        let error = parse_url(url).unwrap_err();
        assert!(!format!("{error} {error:?}").contains(secret), "{error:?}");
    }
    let parsed = parse_url(&format!("postgres://u:{secret}@h/d")).unwrap();
    assert!(!format!("{parsed:?}").contains(secret));
}

#[test]
fn into_profile_applies_env_defaults_and_keeps_secret_apart() {
    let parsed = parse_url("postgres://u:pw@prod-db/app?sslmode=verify-ca").unwrap();
    let (profile, password) = parsed.into_profile("App prod", EnvTag::new(EnvKind::Prod));
    assert!(profile.safety.confirm_writes);
    assert!(!profile.safety.read_only);
    assert_eq!(profile.tls.mode, TlsMode::VerifyCa);
    assert!(profile.secrets.password.is_none());
    assert_eq!(password.unwrap().expose(), "pw");
}

#[test]
fn mysql_and_mariadb_urls_name_their_engine_and_default_port() {
    let parsed = parse_url("mysql://app:pw@db.example.com/shop?ssl-mode=VERIFY_CA").unwrap();
    assert_eq!(parsed.driver, Driver::Mysql);
    assert_eq!(
        (parsed.params.host.as_str(), parsed.params.port),
        ("db.example.com", 3306)
    );
    assert_eq!(parsed.params.database.as_deref(), Some("shop"));
    assert_eq!(parsed.tls_mode, TlsMode::VerifyCa);
    assert_eq!(parsed.password.unwrap().expose(), "pw");
    let maria = parse_url("mariadb://root@127.0.0.1:3307?sslmode=disable").unwrap();
    assert_eq!(maria.driver, Driver::Mariadb);
    assert_eq!(maria.params.port, 3307);
    assert_eq!(maria.tls_mode, TlsMode::Disable);
    let identity = parse_url("mysql://h/d?ssl-mode=verify_identity").unwrap();
    assert_eq!(identity.tls_mode, TlsMode::VerifyFull);
    let preferred = parse_url("mysql://localhost").unwrap();
    assert_eq!(preferred.tls_mode, TlsMode::Prefer);
    assert!(matches!(
        parse_url("mysql://h/d?ssl-mode=SOMETIMES"),
        Err(UrlError::InvalidSslMode { .. })
    ));
}

#[test]
fn sqlite_urls_name_an_absolute_file() {
    for url in ["sqlite:/data/app.db", "sqlite:///data/app.db"] {
        let parsed = parse_url(url).unwrap();
        assert_eq!(parsed.driver, Driver::Sqlite, "{url}");
        let file = parsed.file.unwrap();
        assert_eq!(file.path.to_str(), Some("/data/app.db"));
        assert!(!file.read_only);
        assert_eq!(parsed.tls_mode, TlsMode::Disable);
    }
    let spaced = parse_url("sqlite:/tmp/my%20data.sqlite?mode=ro").unwrap();
    let file = spaced.file.unwrap();
    assert_eq!(file.path.to_str(), Some("/tmp/my data.sqlite"));
    assert!(file.read_only);
    for bad in ["sqlite:relative.db", "sqlite://server/data/app.db"] {
        assert!(
            matches!(parse_url(bad), Err(UrlError::InvalidFilePath)),
            "{bad}"
        );
    }
    let (profile, password) = parse_url("sqlite:/data/app.db")
        .unwrap()
        .into_profile("App", EnvTag::new(EnvKind::Local));
    assert_eq!(profile.driver, Driver::Sqlite);
    assert!(password.is_none());
    assert!(profile.validate().is_ok());
}

#[test]
fn sqlite_memory_urls_open_an_in_memory_database() {
    for url in ["sqlite::memory:", " SQLite://:memory: "] {
        let parsed = parse_url(url).unwrap();
        assert_eq!(parsed.driver, Driver::Sqlite, "{url}");
        let file = parsed.file.clone().unwrap();
        assert!(file.memory && file.path.as_os_str().is_empty(), "{url}");
        let (profile, _) = parsed.into_profile("Scratch", EnvTag::new(EnvKind::Local));
        assert!(profile.validate().is_ok(), "{url}");
    }
    assert!(matches!(
        parse_url("sqlite::memory"),
        Err(UrlError::InvalidFilePath)
    ));
}

#[test]
fn sql_server_urls_map_encrypt_options_to_tls_modes() {
    let parsed =
        parse_url("sqlserver://sa:pw@db.example.com/shop?encrypt=true&trustServerCertificate=true")
            .unwrap();
    assert_eq!(parsed.driver, Driver::Sqlserver);
    assert_eq!(
        (parsed.params.host.as_str(), parsed.params.port),
        ("db.example.com", 1433)
    );
    assert_eq!(parsed.params.database.as_deref(), Some("shop"));
    assert_eq!(parsed.tls_mode, TlsMode::Require);
    assert_eq!(parsed.password.unwrap().expose(), "pw");
    let mssql = parse_url("mssql://app@10.0.0.5:14330?Encrypt=false").unwrap();
    assert_eq!(mssql.driver, Driver::Sqlserver);
    assert_eq!(mssql.params.port, 14330);
    assert_eq!(mssql.tls_mode, TlsMode::Allow);
    let verified = parse_url("sqlserver://h?databaseName=d&encrypt=mandatory").unwrap();
    assert_eq!(verified.params.database.as_deref(), Some("d"));
    assert_eq!(verified.tls_mode, TlsMode::VerifyFull);
    let explicit = parse_url("sqlserver://h?encrypt=false&sslmode=require").unwrap();
    assert_eq!(explicit.tls_mode, TlsMode::Require);
    assert!(matches!(
        parse_url("sqlserver://h?encrypt=sometimes"),
        Err(UrlError::InvalidSslMode { .. })
    ));
}

#[test]
fn jdbc_sql_server_urls_are_parsed() {
    let parsed = parse_url(
        "jdbc:sqlserver://db.example.com:1444;databaseName=shop;user=app;\
         password={p;w};encrypt=true;trustServerCertificate=true;applicationName=Tool",
    )
    .unwrap();
    assert_eq!(parsed.driver, Driver::Sqlserver);
    assert_eq!(
        (parsed.params.host.as_str(), parsed.params.port),
        ("db.example.com", 1444)
    );
    assert_eq!(parsed.params.database.as_deref(), Some("shop"));
    assert_eq!(parsed.params.user.as_deref(), Some("app"));
    assert_eq!(parsed.params.application_name.as_deref(), Some("Tool"));
    assert_eq!(parsed.password.unwrap().expose(), "p;w");
    assert_eq!(parsed.tls_mode, TlsMode::Require);
    let bare = parse_url("jdbc:sqlserver://;serverName=h;portNumber=1500").unwrap();
    assert_eq!((bare.params.host.as_str(), bare.params.port), ("h", 1500));
    assert_eq!(bare.tls_mode, TlsMode::VerifyFull);
    let named = parse_url("jdbc:sqlserver://h\\SQLEXPRESS:1435").unwrap();
    assert_eq!(named.params.port, 1435);
    assert!(matches!(
        parse_url("jdbc:sqlserver://h\\SQLEXPRESS;databaseName=d"),
        Err(UrlError::NamedInstance { .. })
    ));
}
