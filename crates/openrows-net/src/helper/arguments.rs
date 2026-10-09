// SPDX-License-Identifier: Apache-2.0
//! Exact documented CLI vectors; no arbitrary commands or shell expansion.
use super::{CloudFlavor, HelperError, HelperHop};

impl HelperHop {
    /// Build the helper arguments for an ephemeral loopback port.
    ///
    /// # Errors
    /// Invalid route values or a zero local port.
    pub fn arguments(&self, local_port: u16) -> Result<Vec<String>, HelperError> {
        self.validate()?;
        if local_port == 0 {
            return Err(HelperError::Invalid("Local port must be nonzero".into()));
        }
        Ok(match self {
            Self::Ssm { .. } => self.ssm_arguments(local_port),
            Self::Kubectl {
                context,
                namespace,
                resource,
                remote_port,
                kubeconfig,
            } => {
                let mut args = strings(&["--context", context, "--namespace", namespace]);
                optional(&mut args, "--kubeconfig", kubeconfig.as_deref());
                args.extend(strings(&[
                    "port-forward",
                    "--address",
                    "127.0.0.1",
                    resource,
                    &format!("{local_port}:{remote_port}"),
                ]));
                args
            }
            Self::Cloud {
                instance,
                flavor,
                iam_auth,
                private_ip,
                credentials_file,
            } => {
                let mut args =
                    strings(&["--address", "127.0.0.1", "--port", &local_port.to_string()]);
                if *iam_auth {
                    args.push("--auto-iam-authn".into());
                }
                match flavor {
                    CloudFlavor::Cloudsql if *private_ip => args.push("--private-ip".into()),
                    CloudFlavor::Alloydb if !private_ip => args.push("--public-ip".into()),
                    CloudFlavor::Cloudsql | CloudFlavor::Alloydb => {}
                }
                optional(&mut args, "--credentials-file", credentials_file.as_deref());
                args.push(instance.clone());
                args
            }
            Self::Cloudflared { hostname } => strings(&[
                "access",
                "tcp",
                "--hostname",
                hostname,
                "--url",
                &format!("127.0.0.1:{local_port}"),
            ]),
        })
    }

    fn ssm_arguments(&self, local_port: u16) -> Vec<String> {
        let Self::Ssm {
            target,
            remote_host,
            remote_port,
            profile,
            region,
        } = self
        else {
            return Vec::new();
        };
        let document = if remote_host.is_some() {
            "AWS-StartPortForwardingSessionToRemoteHost"
        } else {
            "AWS-StartPortForwardingSession"
        };
        let mut parameters = serde_json::json!({"portNumber": [remote_port.to_string()], "localPortNumber": [local_port.to_string()]});
        if let Some(host) = remote_host {
            parameters["host"] = serde_json::json!([host]);
        }
        let mut args = strings(&[
            "ssm",
            "start-session",
            "--target",
            target,
            "--document-name",
            document,
            "--parameters",
            &parameters.to_string(),
        ]);
        optional(&mut args, "--profile", profile.as_deref());
        optional(&mut args, "--region", region.as_deref());
        args
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
fn optional(args: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        args.extend(strings(&[flag, value]));
    }
}
