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
        host: " localhost ".to_owned(),
        port: 5432,
        database: Some(String::new()),
        user: Some("app".to_owned()),
        tls_mode: TlsModeDto::Prefer,
        tls_files: TlsFilesInput::default(),
        ssh: None,
        safety: SafetyDto {
            read_only: false,
            confirm_writes: true,
        },
        password: PasswordInput::Keep,
    }
}

#[test]
fn apply_trims_fields_and_drops_blank_options() -> Result<(), IpcError> {
    let mut profile =
        ConnectionProfile::new_postgres("x", PgParams::default(), EnvTag::new(EnvKind::Dev));
    input(EnvKindDto::Staging).apply(&mut profile)?;
    assert_eq!(profile.name, "Local PG");
    assert_eq!(profile.group, None);
    assert_eq!(profile.params.host, "localhost");
    assert_eq!(profile.params.database, None);
    assert_eq!(profile.params.user.as_deref(), Some("app"));
    assert_eq!(profile.env_tag.kind, EnvKind::Staging);
    assert!(profile.safety.confirm_writes);
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
    let parsed = parse_postgres_url("postgres://app:s3cret-synthetic@db.test:6543/orders")?;
    let json = serde_json::to_string(&UrlDraft::from(parsed))?;
    assert!(!json.contains("s3cret-synthetic"));
    assert!(json.contains("\"passwordProvided\":true"));
    assert!(json.contains("\"port\":6543"));
    Ok(())
}

#[test]
fn profile_dto_has_no_secret_fields() -> Result<(), serde_json::Error> {
    let profile =
        ConnectionProfile::new_postgres("p", PgParams::default(), EnvTag::new(EnvKind::Prod));
    let json = serde_json::to_value(ProfileDto::from(&profile))?;
    assert_eq!(json["hasPassword"], false);
    assert_eq!(json["safety"]["confirmWrites"], true);
    assert_eq!(json["env"]["kind"], "prod");
    assert!(json.get("password").is_none());
    Ok(())
}
