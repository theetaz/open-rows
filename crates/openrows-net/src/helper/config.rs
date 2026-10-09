// SPDX-License-Identifier: Apache-2.0
//! Structured, secret-free cloud routes; values are arguments, never shell text.
use serde::{Deserialize, Serialize};

/// Cloud proxy product.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "helper-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum CloudFlavor {
    /// Cloud SQL Auth Proxy v2.
    Cloudsql,
    /// `AlloyDB` Auth Proxy.
    Alloydb,
}

/// One structured helper hop, before database TLS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "helper-types", derive(specta::Type))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HelperHop {
    /// A locally trusted executable with literal argument templates.
    Custom {
        /// Absolute executable path.
        program: String,
        /// Literal templates with local and database destination placeholders.
        args: Vec<String>,
        /// Readiness deadline; absent uses 30 seconds.
        #[serde(default)]
        ready_timeout_seconds: Option<u16>,
    },
    /// AWS Session Manager port forwarding.
    Ssm {
        /// Managed node or ECS target.
        target: String,
        /// Remote host; absent forwards a port on the managed node.
        remote_host: Option<String>,
        /// Remote database port.
        remote_port: u16,
        /// AWS named credential profile.
        profile: Option<String>,
        /// AWS region.
        region: Option<String>,
    },
    /// Kubernetes port forwarding.
    Kubectl {
        /// Kubeconfig context.
        context: String,
        /// Kubernetes namespace.
        namespace: String,
        /// pod/name, svc/name or deployment/name.
        resource: String,
        /// Remote port.
        remote_port: u16,
        /// Optional kubeconfig file.
        kubeconfig: Option<String>,
    },
    /// Google managed database auth proxy.
    Cloud {
        /// Connection name or `AlloyDB` instance URI.
        instance: String,
        /// Proxy product.
        flavor: CloudFlavor,
        /// Automatic IAM database authentication.
        iam_auth: bool,
        /// Use private networking (`AlloyDB` uses private IP by default).
        private_ip: bool,
        /// Credential file path; credentials are never stored here.
        credentials_file: Option<String>,
    },
    /// Cloudflare Access TCP forwarding; authentication stays in its own config.
    Cloudflared {
        /// Access application hostname.
        hostname: String,
    },
}

impl HelperHop {
    /// The required executable name.
    #[must_use]
    pub fn tool(&self) -> &'static str {
        match self {
            Self::Custom { .. } => "custom command",
            Self::Ssm { .. } => "aws",
            Self::Kubectl { .. } => "kubectl",
            Self::Cloud {
                flavor: CloudFlavor::Cloudsql,
                ..
            } => "cloud-sql-proxy",
            Self::Cloud {
                flavor: CloudFlavor::Alloydb,
                ..
            } => "alloydb-auth-proxy",
            Self::Cloudflared { .. } => "cloudflared",
        }
    }

    /// Status route label.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Custom { program, .. } => format!("Custom command {program}"),
            Self::Ssm { target, .. } => format!("AWS SSM {target}"),
            Self::Kubectl { resource, .. } => format!("kubectl port-forward {resource}"),
            Self::Cloud { instance, .. } => format!("{} {instance}", self.tool()),
            Self::Cloudflared { hostname } => format!("cloudflared {hostname}"),
        }
    }

    /// Authentication and configuration advice for this hop.
    #[must_use]
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Custom { .. } => {
                "The command exited or never listened on {local_port}; check the program and argument templates."
            }
            Self::Ssm { .. } => {
                "Check the Session Manager plugin in Settings ▸ Tools, region and target; run aws sso login for your profile."
            }
            Self::Kubectl { .. } => {
                "Check kubectl context, namespace and resource; run kubectl config get-contexts."
            }
            Self::Cloud { .. } => {
                "Check instance and credentials; run gcloud auth login and gcloud auth application-default login."
            }
            Self::Cloudflared { .. } => {
                "Check Access policy and hostname; run cloudflared access login https://<hostname>."
            }
        }
    }
}
