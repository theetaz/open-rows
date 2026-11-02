// SPDX-License-Identifier: Apache-2.0
//! Argument vectors, engine capabilities, route security and error classes.
use super::*;
use crate::profile::cloud_auth_error::classify;
use crate::profile::{CloudInvocation, EnvKind, EnvTag, PasswordStorage, ServerParams};
use std::path::Path;

const HOST: &str = "db.abc123.us-east-1.rds.amazonaws.com";

fn profile(driver: Driver, host: &str, auth: CloudAuth) -> ConnectionProfile {
    let params = ServerParams {
        host: host.to_owned(),
        port: 5432,
        user: Some("iam_user".to_owned()),
        ..ServerParams::default()
    };
    let mut profile = ConnectionProfile::new("p", driver, params, EnvTag::new(EnvKind::Dev));
    profile.password_storage = PasswordStorage::Cloud;
    profile.cloud_auth = Some(auth);
    profile.tls.mode = TlsMode::VerifyFull;
    profile
}

fn aws() -> CloudAuth {
    CloudAuth::Aws {
        region: "us-east-1".into(),
        profile: None,
    }
}

fn args(profile: &ConnectionProfile) -> Vec<String> {
    profile
        .cloud_auth
        .as_ref()
        .and_then(|auth| auth.arguments(profile).ok())
        .unwrap_or_default()
}

#[test]
fn aws_arguments_are_one_literal_vector_with_no_secret() {
    let mut auth = aws();
    if let CloudAuth::Aws { profile, .. } = &mut auth {
        *profile = Some("prod admin; $(x)".into());
    }
    assert_eq!(
        args(&profile(Driver::Postgres, HOST, auth)),
        [
            "rds",
            "generate-db-auth-token",
            "--hostname",
            HOST,
            "--port",
            "5432",
            "--username",
            "iam_user",
            "--region",
            "us-east-1",
            "--profile",
            "prod admin; $(x)",
            "--output",
            "text",
        ]
    );
}

#[test]
fn azure_uses_the_database_audience_of_the_engine() {
    let auth = CloudAuth::Azure {
        tenant: Some("contoso.onmicrosoft.com".into()),
    };
    assert_eq!(
        args(&profile(
            Driver::Postgres,
            "x.postgres.database.azure.com",
            auth.clone()
        )),
        [
            "account",
            "get-access-token",
            "--resource-type",
            "oss-rdbms",
            "--tenant",
            "contoso.onmicrosoft.com",
            "--query",
            "accessToken",
            "--output",
            "tsv",
        ]
    );
    let sql = args(&profile(Driver::Sqlserver, "x.database.windows.net", auth));
    assert_eq!(&sql[2..4], ["--resource", "https://database.windows.net/"]);
}

#[test]
fn google_matches_the_installed_gcloud_synopsis() {
    let auth = CloudAuth::Google {
        account: Some("me@example.com".into()),
    };
    assert_eq!(
        args(&profile(Driver::Mysql, "10.0.0.5", auth)),
        [
            "sql",
            "generate-login-token",
            "--quiet",
            "--account",
            "me@example.com"
        ]
    );
}

#[test]
fn each_method_is_offered_only_for_its_engines() {
    let azure = CloudAuth::Azure { tenant: None };
    let google = CloudAuth::Google { account: None };
    for driver in [Driver::Postgres, Driver::Mysql] {
        assert!(aws().supports(driver) && azure.supports(driver) && google.supports(driver));
    }
    assert!(azure.supports(Driver::Sqlserver));
    assert!(!aws().supports(Driver::Sqlserver) && !google.supports(Driver::Sqlserver));
    for driver in [
        Driver::Mariadb,
        Driver::Sqlite,
        Driver::Clickhouse,
        Driver::Mongodb,
        Driver::Libsql,
        Driver::LibsqlFile,
        Driver::Duckdb,
    ] {
        assert!(!aws().supports(driver) && !azure.supports(driver) && !google.supports(driver));
    }
}

#[test]
fn validation_refuses_flags_bad_regions_sockets_and_other_engines() {
    let flag = CloudAuth::Aws {
        region: "us-east-1".into(),
        profile: Some("--debug".into()),
    };
    let region = CloudAuth::Aws {
        region: "US East".into(),
        profile: None,
    };
    let tenant = CloudAuth::Azure {
        tenant: Some("a\nb".into()),
    };
    for auth in [flag, region, tenant] {
        assert!(profile(Driver::Postgres, HOST, auth).validate().is_err());
    }
    assert!(profile(Driver::Sqlserver, HOST, aws()).validate().is_err());
    assert!(
        profile(Driver::Postgres, "/var/run/postgresql", aws())
            .validate()
            .is_err()
    );
    let mut missing = profile(Driver::Postgres, HOST, aws());
    missing.cloud_auth = None;
    assert!(missing.validate().is_err());
    assert!(profile(Driver::Postgres, HOST, aws()).validate().is_ok());
    let mut option_like = profile(Driver::Postgres, HOST, aws());
    option_like.params.user = Some("--profile=evil".into());
    assert!(CloudInvocation::new(&option_like, Some(Path::new("/bin/sh")), &[]).is_err());
}

#[test]
fn only_verified_tls_or_the_google_auth_proxy_may_carry_a_token() {
    let refused = |mode, cloud_ca, auth: CloudAuth, helper: Option<HelperHop>| {
        let mut profile = profile(Driver::Postgres, HOST, auth.clone());
        profile.tls.mode = mode;
        profile.tls.cloud_ca = cloud_ca;
        profile.helper = helper;
        auth.require_secure_route(&profile).is_err()
    };
    for mode in [
        TlsMode::Disable,
        TlsMode::Allow,
        TlsMode::Prefer,
        TlsMode::Require,
    ] {
        assert!(refused(mode, None, aws(), None), "{mode:?}");
    }
    // `prefer` may fall back to plaintext even with a CA; loopback is no exemption.
    assert!(refused(
        TlsMode::Prefer,
        Some(crate::profile::CloudCa::AwsRds),
        aws(),
        None
    ));
    for mode in [TlsMode::VerifyCa, TlsMode::VerifyFull] {
        assert!(!refused(mode, None, aws(), None), "{mode:?}");
    }
    assert!(!refused(
        TlsMode::Require,
        Some(crate::profile::CloudCa::AwsRds),
        aws(),
        None
    ));
    let google = CloudAuth::Google { account: None };
    let proxy = |flavor| {
        Some(HelperHop::Cloud {
            instance: "project:region:instance".into(),
            flavor,
            iam_auth: false,
            private_ip: true,
            credentials_file: None,
        })
    };
    assert!(!refused(
        TlsMode::Disable,
        None,
        google.clone(),
        proxy(CloudFlavor::Cloudsql)
    ));
    let mut mysql = profile(Driver::Mysql, "10.0.0.5", google.clone());
    mysql.tls.mode = TlsMode::Disable;
    mysql.helper = proxy(CloudFlavor::Cloudsql);
    assert!(
        google.require_secure_route(&mysql).is_err(),
        "cleartext plugin needs TLS"
    );
    assert!(refused(
        TlsMode::Disable,
        None,
        google,
        proxy(CloudFlavor::Alloydb)
    ));
    assert!(refused(
        TlsMode::Disable,
        None,
        aws(),
        proxy(CloudFlavor::Cloudsql)
    ));
    assert!(refused(
        TlsMode::Disable,
        None,
        CloudAuth::Azure { tenant: None },
        proxy(CloudFlavor::Cloudsql)
    ));
}

#[test]
fn an_insecure_route_says_what_to_change_before_anything_runs() {
    let mut insecure = profile(Driver::Postgres, HOST, aws());
    insecure.tls.mode = TlsMode::Disable;
    let error = CloudInvocation::new(&insecure, Some(Path::new("/does/not/exist")), &[])
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(error.contains("Verify full"), "{error}");
    assert!(error.contains("AWS RDS"), "{error}");
}

#[test]
fn a_region_the_host_contradicts_is_named() {
    let wrong = CloudAuth::Aws {
        region: "eu-west-1".into(),
        profile: None,
    };
    let error = CloudInvocation::new(
        &profile(Driver::Postgres, HOST, wrong),
        Some(Path::new("/bin/sh")),
        &[],
    )
    .err()
    .map(|error| error.to_string())
    .unwrap_or_default();
    assert!(
        error.contains("us-east-1") && error.contains("eu-west-1"),
        "{error}"
    );
    assert_eq!(host_region(HOST), Some("us-east-1"));
    assert_eq!(
        host_region("a.proxy-b.ap-south-1.rds.amazonaws.com.cn"),
        Some("ap-south-1")
    );
    assert_eq!(host_region("db.example.com"), None);
}

#[test]
fn cli_messages_become_typed_errors() {
    let kind = |text: &str| classify("x", "x login", text);
    for text in [
        "Unable to locate credentials. You can configure credentials by running \"aws configure\".",
        "Error loading SSO Token: Token for dev does not exist",
        "The SSO session associated with this profile has expired or is otherwise invalid.",
        "An error occurred (ExpiredToken): The security token included in the request is expired",
        "ERROR: Please run 'az login' to setup account.",
        "AADSTS700082: The refresh token has expired due to inactivity.",
        "ERROR: (gcloud.sql.generate-login-token) You do not currently have an active account selected.",
        "ERROR: There was a problem refreshing your current auth tokens: invalid_grant",
    ] {
        assert!(
            matches!(kind(text), CloudAuthError::SignedOut { .. }),
            "{text}"
        );
    }
    for text in [
        "The config profile (nope) could not be found",
        "You must specify a region. You can also configure your region by running \"aws configure\".",
        "AADSTS90002: Tenant 'nope' not found.",
    ] {
        assert!(
            matches!(kind(text), CloudAuthError::WrongTarget { .. }),
            "{text}"
        );
    }
    for text in [
        "An error occurred (AccessDenied) when calling the operation",
        "ERROR: (gcloud) PERMISSION_DENIED: not allowed",
        "AADSTS65001: The user has not consented",
    ] {
        assert!(
            matches!(kind(text), CloudAuthError::PermissionDenied { .. }),
            "{text}"
        );
    }
    assert!(matches!(
        kind("network is on fire"),
        CloudAuthError::Failed { .. }
    ));
}

#[test]
fn credential_like_lines_never_reach_an_error() {
    let jwt = "eyJhbGciOiJSUzI1NiJ9.payload.signature";
    let text = format!(
        "boom {jwt}\nhttps://h/?X-Amz-Security-Token=secret-part\naccessToken: secret-two\nordinary"
    );
    let rendered = classify("az", "az login", &text).to_string();
    assert!(rendered.contains("ordinary"), "{rendered}");
    for secret in [jwt, "secret-part", "secret-two"] {
        assert!(!rendered.contains(secret), "{rendered}");
    }
}
