// SPDX-License-Identifier: Apache-2.0
use super::*;

pub(crate) fn input(env: EnvKindDto) -> ProfileInput {
    ProfileInput {
        name: " Local PG ".to_owned(),
        group: Some("  ".to_owned()),
        env: EnvInput {
            kind: env,
            label: None,
            color: None,
        },
        engine: EngineDto::Postgres,
        file: None,
        host: " localhost ".to_owned(),
        port: 5432,
        database: Some(String::new()),
        user: Some("app".to_owned()),
        tls_mode: TlsModeDto::Prefer,
        tls_files: TlsFilesInput::default(),
        ssh: None,
        proxy: None,
        safety: SafetyDto {
            read_only: false,
            confirm_writes: true,
        },
        record_history: false,
        startup_commands: "  SET search_path TO app;\n".into(),
        password_storage: PasswordStorageDto::Keychain,
        credential_command: None,
        tags: Vec::new(),
        password: PasswordInput::Keep,
        limits: LimitsDto::default(),
    }
}

#[test]
fn apply_trims_fields_and_drops_blank_options() -> Result<(), IpcError> {
    let mut profile =
        ConnectionProfile::new_postgres("x", ServerParams::default(), EnvTag::new(EnvKind::Dev));
    input(EnvKindDto::Staging).apply(&mut profile)?;
    assert_eq!(profile.name, "Local PG");
    assert_eq!(profile.group, None);
    assert_eq!(profile.params.host, "localhost");
    assert_eq!(profile.params.database, None);
    assert_eq!(profile.params.user.as_deref(), Some("app"));
    assert_eq!(profile.env_tag.kind, EnvKind::Staging);
    assert!(profile.safety.confirm_writes);
    assert_eq!(profile.startup_commands, "SET search_path TO app;");
    Ok(())
}

#[test]
fn custom_environment_needs_a_label_and_a_hex_colour() {
    let mut env = EnvInput {
        kind: EnvKindDto::Custom,
        label: None,
        color: None,
    };
    assert!(env_tag(&env).is_err());
    env.label = Some("qa".to_owned());
    assert_eq!(env_tag(&env).map(|tag| tag.color.len()), Ok(7));
    env.color = Some("red".to_owned());
    assert!(env_tag(&env).is_err());
    env.color = Some("#0a0B0c".to_owned());
    assert_eq!(env_tag(&env).map(|tag| tag.color), Ok("#0a0B0c".to_owned()));
}

#[test]
fn url_draft_reports_but_never_contains_the_password() -> Result<(), Box<dyn std::error::Error>> {
    let parsed = parse_url("postgres://app:s3cret-synthetic@db.test:6543/orders")?;
    let json = serde_json::to_string(&UrlDraft::from(parsed))?;
    assert!(!json.contains("s3cret-synthetic"));
    assert!(json.contains("\"passwordProvided\":true"));
    assert!(json.contains("\"port\":6543"));
    Ok(())
}

#[test]
fn profile_dto_has_no_secret_fields() -> Result<(), serde_json::Error> {
    let profile =
        ConnectionProfile::new_postgres("p", ServerParams::default(), EnvTag::new(EnvKind::Prod));
    let json = serde_json::to_value(ProfileDto::from(&profile))?;
    assert_eq!(json["hasPassword"], false);
    assert_eq!(json["safety"]["confirmWrites"], true);
    assert_eq!(json["env"]["kind"], "prod");
    assert!(json.get("password").is_none());
    Ok(())
}

#[test]
fn file_engines_keep_their_file_and_drop_any_route() -> Result<(), IpcError> {
    let mut form = input(EnvKindDto::Local);
    form.engine = EngineDto::Sqlite;
    form.file = Some(FileDto {
        path: " /data/app.db ".to_owned(),
        read_only: true,
        ..FileDto::default()
    });
    form.tls_mode = TlsModeDto::VerifyFull;
    form.ssh = Some(super::super::route_dto::tests::ssh_input(
        super::super::route_dto::SshAuthDto::Agent,
    ));
    let mut profile = form.new_profile();
    form.apply(&mut profile)?;
    assert_eq!(profile.driver, openrows_store::profile::Driver::Sqlite);
    let file = profile.file.clone().unwrap_or_default();
    assert_eq!(file.path, std::path::PathBuf::from("/data/app.db"));
    assert!(file.read_only);
    assert!(profile.ssh.is_none());
    assert_eq!(profile.tls.mode, TlsMode::Disable);
    assert!(profile.validate().is_ok());
    let dto = serde_json::to_value(ProfileDto::from(&profile))
        .map_err(|e| IpcError::invalid(e.to_string()))?;
    assert_eq!(dto["engine"], "sqlite");
    assert_eq!(dto["file"]["readOnly"], true);
    Ok(())
}

#[test]
fn saving_the_form_keeps_attachments_and_memory_drops_file_options() -> Result<(), IpcError> {
    let mut form = input(EnvKindDto::Local);
    form.engine = EngineDto::Sqlite;
    form.file = Some(FileDto {
        path: "/data/app.db".to_owned(),
        ..FileDto::default()
    });
    let mut profile = form.new_profile();
    form.apply(&mut profile)?;
    let attached = openrows_store::profile::AttachedFile {
        alias: "logs".to_owned(),
        path: "/data/logs.db".into(),
    };
    if let Some(file) = profile.file.as_mut() {
        file.attach(attached.clone())?;
    }
    form.file = Some(FileDto {
        path: "/ignored.db".to_owned(),
        memory: true,
        read_only: true,
        create: true,
        attached: Vec::new(),
    });
    form.apply(&mut profile)?;
    let file = profile.file.clone().unwrap_or_default();
    assert!(file.memory && !file.read_only && !file.create);
    assert!(file.path.as_os_str().is_empty());
    assert_eq!(file.attached, vec![attached]);
    assert!(profile.validate().is_ok());
    let dto = serde_json::to_value(ProfileDto::from(&profile))
        .map_err(|e| IpcError::invalid(e.to_string()))?;
    assert_eq!(dto["file"]["memory"], true);
    assert_eq!(dto["file"]["attached"][0]["alias"], "logs");
    Ok(())
}

#[test]
fn forms_without_an_engine_are_postgres() -> Result<(), serde_json::Error> {
    let form: ProfileInput = serde_json::from_value(serde_json::json!({
        "name": "Old", "group": null,
        "env": { "kind": "local", "label": null, "color": null },
        "host": "localhost", "port": 5432, "database": null, "user": "app",
        "tlsMode": "prefer",
        "safety": { "readOnly": false, "confirmWrites": false },
        "password": { "kind": "keep" }
    }))?;
    assert_eq!(form.engine, EngineDto::Postgres);
    assert!(form.file.is_none());
    assert!(
        form.record_history,
        "history is recorded unless switched off"
    );
    assert!(form.startup_commands.is_empty());
    Ok(())
}
