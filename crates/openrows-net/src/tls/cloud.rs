// SPDX-License-Identifier: Apache-2.0
//! Public CA certificates of managed database services, bundled so that
//! `verify-ca` and `verify-full` work without downloading a CA file
//! (docs/24 §4). Sources, fingerprints and the refresh process: docs/24 §13.

use std::fmt;

/// A bundled cloud provider CA set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloudCa {
    /// Amazon RDS / Aurora global bundle (every commercial region's root CAs).
    AwsRds,
    /// Azure Database for `PostgreSQL` / `MySQL`: `DigiCert` Global Root G2,
    /// `DigiCert` Global Root CA and Microsoft RSA Root CA 2017.
    Azure,
}

impl CloudCa {
    /// Every bundled set.
    pub const ALL: [Self; 2] = [Self::AwsRds, Self::Azure];

    /// The PEM certificates.
    #[must_use]
    pub fn pem(self) -> &'static [u8] {
        match self {
            Self::AwsRds => include_bytes!("../../certs/aws-rds-global-bundle.pem"),
            Self::Azure => include_bytes!("../../certs/azure-database-roots.pem"),
        }
    }
}

impl fmt::Display for CloudCa {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AwsRds => "AWS RDS CA bundle",
            Self::Azure => "Azure Database CA bundle",
        })
    }
}
