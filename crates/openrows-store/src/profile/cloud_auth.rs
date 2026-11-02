// SPDX-License-Identifier: Apache-2.0
//! Short-lived database passwords from the cloud CLI the user already signed
//! in to (FR-CONN-11, docs/24 §19). The profile keeps only non-secret settings.
//! The token is read from the CLI's stdout for one connect and is never stored,
//! logged or passed on a command line.
use super::{ConnectionProfile, Driver, HelperHop, ProfileError, TlsMode};
use crate::profile::cloud_auth_error::CloudAuthError;
use openrows_net::helper::CloudFlavor;
use serde::{Deserialize, Serialize};

const FIELD_LIMIT: usize = 256;
const AZURE_SQL_RESOURCE: &str = "https://database.windows.net/";

/// How a profile signs in with a cloud token. Holds settings only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "provider",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CloudAuth {
    /// RDS and Aurora IAM database authentication.
    Aws {
        /// Region the token is signed for.
        region: String,
        /// Named profile; `None` uses the CLI's default credentials.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
    },
    /// Microsoft Entra ID access tokens.
    Azure {
        /// Tenant; `None` uses the signed-in default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tenant: Option<String>,
    },
    /// Cloud SQL IAM database authentication.
    Google {
        /// Account; `None` uses the active gcloud account.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account: Option<String>,
    },
}

impl CloudAuth {
    /// The executable that produces the token.
    #[must_use]
    pub fn tool(&self) -> &'static str {
        match self {
            Self::Aws { .. } => "aws",
            Self::Azure { .. } => "az",
            Self::Google { .. } => "gcloud",
        }
    }

    /// Whether the provider issues tokens this engine can log in with.
    #[must_use]
    pub fn supports(&self, driver: Driver) -> bool {
        match self {
            Self::Aws { .. } | Self::Google { .. } => {
                matches!(driver, Driver::Postgres | Driver::Mysql)
            }
            Self::Azure { .. } => {
                matches!(driver, Driver::Postgres | Driver::Mysql | Driver::Sqlserver)
            }
        }
    }

    fn validate(&self) -> Result<(), ProfileError> {
        match self {
            Self::Aws { region, profile } => {
                if !is_region(region) {
                    return invalid("cloudAuth.region", "use a region such as us-east-1");
                }
                check_option("cloudAuth.profile", profile.as_deref())
            }
            Self::Azure { tenant } => check_option("cloudAuth.tenant", tenant.as_deref()),
            Self::Google { account } => check_option("cloudAuth.account", account.as_deref()),
        }
    }

    /// The literal argument vector; each value is one argument, never a shell word.
    pub(super) fn arguments(
        &self,
        profile: &ConnectionProfile,
    ) -> Result<Vec<String>, ProfileError> {
        let mut args: Vec<String> = Vec::new();
        let mut push = |values: &[&str]| args.extend(values.iter().map(|&value| value.to_owned()));
        match self {
            Self::Aws {
                region,
                profile: aws_profile,
            } => {
                let params = &profile.params;
                let user = params.user.as_deref().unwrap_or_default();
                if user.is_empty() {
                    return invalid("params.user", "is required for AWS IAM tokens");
                }
                check_option("params.user", Some(user))?;
                check_option("params.host", Some(&params.host))?;
                let port = params.port.to_string();
                push(&["rds", "generate-db-auth-token"]);
                push(&["--hostname", &params.host, "--port", &port]);
                push(&["--username", user, "--region", region]);
                if let Some(aws_profile) = aws_profile {
                    push(&["--profile", aws_profile]);
                }
                push(&["--output", "text"]);
            }
            Self::Azure { tenant } => {
                push(&["account", "get-access-token"]);
                if profile.driver == Driver::Sqlserver {
                    push(&["--resource", AZURE_SQL_RESOURCE]);
                } else {
                    push(&["--resource-type", "oss-rdbms"]);
                }
                if let Some(tenant) = tenant {
                    push(&["--tenant", tenant]);
                }
                push(&["--query", "accessToken", "--output", "tsv"]);
            }
            Self::Google { account } => {
                push(&["sql", "generate-login-token", "--quiet"]);
                if let Some(account) = account {
                    push(&["--account", account]);
                }
            }
        }
        Ok(args)
    }

    /// Refuse a route that could expose the token or send it to the wrong
    /// party. Only the Cloud SQL Auth Proxy (docs/24 §15) stands in for
    /// database TLS, and only for a Google token on `PostgreSQL`: it encrypts
    /// and authenticates the path to the instance. `MySQL` token logins use
    /// the cleartext plugin, which the driver only allows over TLS, and the
    /// proxy has its own Automatic IAM option for them.
    pub(super) fn require_secure_route(
        &self,
        profile: &ConnectionProfile,
    ) -> Result<(), CloudAuthError> {
        let through_auth_proxy = matches!(
            profile.helper,
            Some(HelperHop::Cloud {
                flavor: CloudFlavor::Cloudsql,
                ..
            })
        );
        if matches!(self, Self::Google { .. })
            && through_auth_proxy
            && profile.driver == Driver::Postgres
        {
            return Ok(());
        }
        let reason = match profile.tls.mode {
            TlsMode::Disable | TlsMode::Allow | TlsMode::Prefer => "it could travel unencrypted",
            _ if profile.tls.skips_verification() => "the server certificate is not verified",
            _ => return Ok(()),
        };
        Err(CloudAuthError::InsecureRoute {
            reason,
            fix: match self {
                Self::Aws { .. } => {
                    "In the connection's TLS section choose Verify full with Cloud CA: AWS RDS / Aurora."
                }
                Self::Azure { .. } => {
                    "In the connection's TLS section choose Verify full with Cloud CA: Azure Database, or the system trust store for Azure SQL."
                }
                Self::Google { .. } => {
                    "Choose TLS Verify CA with the instance's server-ca.pem as the CA file. On PostgreSQL the Cloud SQL Auth Proxy route also works; on MySQL use the proxy's Automatic IAM option with No password instead."
                }
            },
        })
    }

    /// Signing for one region and connecting to another only fails later,
    /// inside the database login, so catch it while it can be named.
    pub(super) fn require_matching_region(&self, host: &str) -> Result<(), CloudAuthError> {
        let Self::Aws { region, .. } = self else {
            return Ok(());
        };
        match host_region(host) {
            Some(expected) if expected != region => Err(CloudAuthError::WrongTarget {
                tool: "aws",
                detail: format!(
                    "The host is in region {expected} but the connection's region is {region}."
                ),
            }),
            _ => Ok(()),
        }
    }
}

impl ConnectionProfile {
    /// A cloud token needs a known provider for this engine and a TCP host.
    pub(super) fn validate_cloud_auth(&self) -> Result<(), ProfileError> {
        let Some(auth) = &self.cloud_auth else {
            return invalid("cloudAuth", "choose a cloud provider");
        };
        if !auth.supports(self.driver) {
            return invalid(
                "cloudAuth",
                "this provider does not issue tokens for this engine",
            );
        }
        if self.params.host.starts_with('/') {
            return invalid("cloudAuth", "cloud tokens need a TCP host, not a socket");
        }
        auth.validate()
    }
}

fn invalid<T>(field: &'static str, reason: &'static str) -> Result<T, ProfileError> {
    Err(ProfileError::Invalid { field, reason })
}

/// A value that cannot be mistaken for a flag or smuggle a control character.
fn check_option(field: &'static str, value: Option<&str>) -> Result<(), ProfileError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_empty()
        || value.starts_with('-')
        || value.len() > FIELD_LIMIT
        || value.chars().any(char::is_control)
    {
        return invalid(
            field,
            "must not be empty, start with - or hold control characters",
        );
    }
    Ok(())
}

fn is_region(region: &str) -> bool {
    !region.is_empty()
        && region.len() <= 32
        && !region.starts_with('-')
        && region
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `<name>.<id>.<region>.rds.amazonaws.com[.cn]`, the shape of every RDS,
/// Aurora and RDS Proxy endpoint; other host names say nothing about a region.
fn host_region(host: &str) -> Option<&str> {
    let rest = host
        .strip_suffix(".rds.amazonaws.com")
        .or_else(|| host.strip_suffix(".rds.amazonaws.com.cn"))?;
    rest.rsplit('.').next().filter(|region| is_region(region))
}

#[cfg(test)]
#[path = "cloud_auth_tests.rs"]
mod tests;
