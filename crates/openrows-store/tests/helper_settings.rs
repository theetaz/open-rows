// SPDX-License-Identifier: Apache-2.0
//! Backward compatibility, disk round trips and unsupported route combinations.
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, HelperHop, ProfileFiles, ProxyKind, ProxySettings,
    ServerParams, SshAuthMethod, SshSettings, parse_url,
};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn profile() -> ConnectionProfile {
    ConnectionProfile::new_postgres("test", ServerParams::default(), EnvTag::new(EnvKind::Dev))
}
fn hop() -> HelperHop {
    HelperHop::Kubectl {
        context: "dev".into(),
        namespace: "default".into(),
        resource: "svc/db".into(),
        remote_port: 5432,
        kubeconfig: None,
    }
}
#[test]
fn old_profiles_and_helper_save_load_round_trip() -> Result {
    let direct = profile();
    let json = serde_json::to_value(&direct)?;
    assert!(json.get("helper").is_none());
    let old: ConnectionProfile = serde_json::from_value(json)?;
    assert_eq!(old, direct);
    let directory = tempfile::tempdir()?;
    let files = ProfileFiles::new(directory.path());
    let mut routed = profile();
    routed.helper = Some(hop());
    files.save(&routed)?;
    assert_eq!(files.load(routed.id)?, routed);
    Ok(())
}
#[test]
fn helper_rejects_ssh_proxy_and_local_files() {
    let mut profile = profile();
    profile.helper = Some(hop());
    profile.ssh = Some(SshSettings {
        host: "jump".into(),
        port: None,
        user: None,
        auth: SshAuthMethod::Agent,
        jump_hosts: Vec::new(),
    });
    assert!(profile.validate().is_err());
    profile.ssh = None;
    profile.proxy = Some(ProxySettings {
        kind: ProxyKind::Socks5,
        host: "proxy".into(),
        port: 1080,
        user: None,
    });
    assert!(profile.validate().is_err());
    profile.proxy = None;
    profile.params.host = "/socket".into();
    assert!(profile.validate().is_err());
}
#[test]
fn database_urls_do_not_represent_helper_routes() -> Result {
    let parsed = parse_url("postgres://app@db.example.invalid/test")?;
    assert_eq!(parsed.params.host, "db.example.invalid");
    // URLs remain database-only: the cloud route is a separate profile property.
    assert!(parse_url("kubectl://svc/db").is_err());
    Ok(())
}
