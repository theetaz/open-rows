// SPDX-License-Identifier: Apache-2.0
//! Public-file parsing, missing files and password import opt-in.
use openrows_secrets::MemorySecretStore;
use openrows_store::profile::{
    ProfileFiles, ProfileService, TlsMode,
    exchange::{self, ImportChoice, ImportDecision},
    tool_import::{self, ToolFormat},
};
use std::sync::Arc;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn pgpass_escaping_comments_wildcards_and_invalid_rows() -> TestResult {
    let cases = [
        (
            "# comment\nlocalhost:5432:app:user:pw\n",
            1,
            0,
            "localhost",
            "pw",
        ),
        (
            r"host\:name:5432:db:user:pa\:ss\\word",
            1,
            0,
            "host:name",
            r"pa:ss\word",
        ),
        ("*:*:*:*:pw\nlocalhost:5432:*:user:pw", 0, 2, "", ""),
        ("bad\n", 0, 1, "", ""),
    ];
    for (text, count, warnings, host, password) in cases {
        let batch = tool_import::parse_pgpass(text)?;
        assert_eq!(batch.entries.len(), count);
        assert_eq!(batch.warnings.len(), warnings);
        if let Some(entry) = batch.entries.first() {
            assert_eq!(entry.profile.params.host, host);
            assert_eq!(
                entry
                    .password
                    .as_ref()
                    .map(openrows_secrets::SecretString::expose),
                Some(password)
            );
        }
    }
    assert!(tool_import::parse_pgpass("host:0:db:user:pw").is_err());
    Ok(())
}
#[test]
fn pg_service_known_fields_unknown_keys_and_tls_paths() -> TestResult {
    let batch = tool_import::parse_pg_service(
        "# comment\n[service]\nhost=localhost\nport=6432\ndbname=app\nuser=user\nsslmode=verify-full\nsslrootcert=/ca.pem\nsslcert=/cert.pem\nsslkey=/key.pem\napplication_name=OpenRows\nconnect_timeout=5\nunknown=test-only\n",
    )?;
    let profile = &batch.entries[0].profile;
    assert_eq!(profile.params.port, 6432);
    assert_eq!(profile.params.database.as_deref(), Some("app"));
    assert_eq!(profile.params.application_name.as_deref(), Some("OpenRows"));
    assert_eq!(profile.params.options["connect_timeout"], "5");
    assert_eq!(profile.tls.mode, TlsMode::VerifyFull);
    assert_eq!(
        profile
            .tls
            .client_key
            .as_ref()
            .and_then(|path| path.to_str()),
        Some("/key.pem")
    );
    assert_eq!(batch.warnings, ["[service]: ignored key unknown"]);
    for value in ["bad", "0", "65536"] {
        assert!(tool_import::parse_pg_service(&format!("[service]\nport={value}")).is_err());
    }
    Ok(())
}
#[test]
fn mysql_groups_quotes_escapes_includes_and_unknown_keys() -> TestResult {
    let text = "!include /other.cnf\n!includedir /other\n[client]\nhost=localhost\nport=3307\nuser=user\npassword=\"pw\\s\\\\\"\ndatabase=app\nssl-ca=/ca.pem\nssl-cert=/cert.pem\nssl-key=/key.pem\nssl-mode=VERIFY_IDENTITY\nignored=x\n[mysql]\nsocket=/tmp/mysql.sock\n[clienttest]\nuser=other\n[mysqld]\npassword=x";
    let batch = tool_import::parse_my_cnf(text)?;
    assert_eq!(batch.entries.len(), 3);
    assert_eq!(batch.warnings.len(), 4);
    let first = &batch.entries[0];
    assert_eq!(
        first
            .password
            .as_ref()
            .map(openrows_secrets::SecretString::expose),
        Some("pw \\")
    );
    assert_eq!(first.profile.params.port, 3307);
    assert_eq!(first.profile.tls.mode, TlsMode::VerifyFull);
    assert_eq!(batch.entries[1].profile.params.host, "/tmp/mysql.sock");
    for (value, expected) in [("'pw#x'", "pw#x"), ("pw # note", "pw"), (r"pw\n", "pw\n")] {
        let batch = tool_import::parse_my_cnf(&format!("[client]\npassword={value}"))?;
        assert_eq!(
            batch.entries[0]
                .password
                .as_ref()
                .map(openrows_secrets::SecretString::expose),
            Some(expected)
        );
    }
    Ok(())
}
#[test]
fn missing_files_are_notes_and_keychain_password_requires_opt_in() -> TestResult {
    let dir = tempfile::tempdir()?;
    for format in [ToolFormat::Pgpass, ToolFormat::PgService, ToolFormat::MyCnf] {
        let missing = tool_import::read(format, &dir.path().join("missing"))?;
        assert!(missing.entries.is_empty());
        assert!(missing.warnings[0].ends_with("not found"));
    }
    let service = ProfileService::new(
        ProfileFiles::new(&dir.path().join("profiles")),
        Arc::new(MemorySecretStore::new()),
    );
    for save_password in [false, true] {
        let batch = tool_import::parse_my_cnf(&format!(
            "[client{}]\npassword=pw",
            usize::from(save_password)
        ))?;
        let saved = exchange::apply_import(
            &service,
            batch,
            &[ImportDecision {
                choice: ImportChoice::KeepBoth,
                save_password,
            }],
        )?;
        assert_eq!(service.password(&saved[0])?.is_some(), save_password);
        assert!(saved[0].imported);
    }
    Ok(())
}
