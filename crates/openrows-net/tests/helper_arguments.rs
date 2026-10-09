// SPDX-License-Identifier: Apache-2.0
//! Golden cloud CLI vectors and secret scrubbing.
use openrows_net::helper::{CloudFlavor, HelperHop, local_endpoint, scrub};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn kube() -> HelperHop {
    HelperHop::Kubectl {
        context: "dev".into(),
        namespace: "default".into(),
        resource: "svc/db".into(),
        remote_port: 5432,
        kubeconfig: Some("/cfg/kube".into()),
    }
}

#[test]
fn kubectl_golden_vector() -> Result {
    assert_eq!(
        kube().arguments(7000)?,
        [
            "--context",
            "dev",
            "--namespace",
            "default",
            "--kubeconfig",
            "/cfg/kube",
            "port-forward",
            "--address",
            "127.0.0.1",
            "svc/db",
            "7000:5432"
        ]
    );
    Ok(())
}
#[test]
fn ssm_golden_vectors() -> Result {
    let mut hop = HelperHop::Ssm {
        target: "i-test".into(),
        remote_host: None,
        remote_port: 5432,
        profile: Some("dev".into()),
        region: Some("test-region".into()),
    };
    assert_eq!(
        hop.arguments(7000)?,
        [
            "ssm",
            "start-session",
            "--target",
            "i-test",
            "--document-name",
            "AWS-StartPortForwardingSession",
            "--parameters",
            "{\"localPortNumber\":[\"7000\"],\"portNumber\":[\"5432\"]}",
            "--profile",
            "dev",
            "--region",
            "test-region"
        ]
    );
    if let HelperHop::Ssm { remote_host, .. } = &mut hop {
        *remote_host = Some("db.internal".into());
    }
    let args = hop.arguments(7000)?;
    assert_eq!(args[5], "AWS-StartPortForwardingSessionToRemoteHost");
    assert_eq!(
        args[7],
        "{\"host\":[\"db.internal\"],\"localPortNumber\":[\"7000\"],\"portNumber\":[\"5432\"]}"
    );
    Ok(())
}
#[test]
fn cloud_proxy_golden_vectors() -> Result {
    for flavor in [CloudFlavor::Cloudsql, CloudFlavor::Alloydb] {
        let hop = HelperHop::Cloud {
            instance: "test-instance".into(),
            flavor,
            iam_auth: true,
            private_ip: flavor == CloudFlavor::Cloudsql,
            credentials_file: Some("/cfg/credentials".into()),
        };
        let flag = if flavor == CloudFlavor::Cloudsql {
            "--private-ip"
        } else {
            "--public-ip"
        };
        assert_eq!(
            hop.arguments(7000)?,
            [
                "--address",
                "127.0.0.1",
                "--port",
                "7000",
                "--auto-iam-authn",
                flag,
                "--credentials-file",
                "/cfg/credentials",
                "test-instance"
            ]
        );
    }
    Ok(())
}
#[test]
fn cloudflared_golden_vector() -> Result {
    let hop = HelperHop::Cloudflared {
        hostname: "db.example.invalid".into(),
    };
    assert_eq!(
        hop.arguments(7000)?,
        [
            "access",
            "tcp",
            "--hostname",
            "db.example.invalid",
            "--url",
            "127.0.0.1:7000"
        ]
    );
    Ok(())
}
#[test]
fn validation_port_and_scrubbing() -> Result {
    assert!(kube().arguments(0).is_err());
    assert!(
        HelperHop::Cloudflared {
            hostname: "--token=x".into()
        }
        .validate()
        .is_err()
    );
    let address = local_endpoint()?;
    assert!(address.ip().is_loopback() && address.port() > 0);
    for text in [
        "token=x",
        "PASSWORD=x",
        "Authorization: Bearer x",
        "token = x",
    ] {
        assert!(!scrub(text).contains('x'));
    }
    assert_eq!(scrub("context not found"), "context not found");
    Ok(())
}

#[test]
fn instance_query_cannot_override_loopback_binding() {
    let hop = HelperHop::Cloud {
        instance: "project:region:instance?address=0.0.0.0&port=1234".into(),
        flavor: CloudFlavor::Cloudsql,
        iam_auth: false,
        private_ip: true,
        credentials_file: None,
    };
    assert!(hop.validate().is_err());
}
