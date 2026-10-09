// SPDX-License-Identifier: Apache-2.0
//! Field validation prevents positional query options from changing the local bind.
use super::{HelperError, HelperHop};
impl HelperHop {
    /// Check required fields and reject option-like/control-character values.
    ///
    /// # Errors
    /// A user-readable field validation failure.
    pub fn validate(&self) -> Result<(), HelperError> {
        if let Self::Custom {
            program,
            args,
            ready_timeout_seconds,
        } = self
        {
            return super::custom::validate(program, args, *ready_timeout_seconds);
        }
        self.validate_destination()?;
        let (required, port) = self.required_fields();
        if port == 0 || required.iter().any(|value| value.trim().is_empty()) {
            return Err(invalid(
                "Cloud route requires all fields and a nonzero remote port",
            ));
        }
        let encoded = serde_json::to_value(self).map_err(|error| invalid(&error.to_string()))?;
        if encoded.as_object().is_some_and(|fields| {
            fields
                .values()
                .filter_map(serde_json::Value::as_str)
                .any(invalid_value)
        }) {
            return Err(invalid(
                "Cloud route values cannot be blank, start with '-' or contain control characters",
            ));
        }
        Ok(())
    }
    fn required_fields(&self) -> (Vec<&str>, u16) {
        match self {
            Self::Custom { program, .. } => (vec![program], 1),
            Self::Ssm {
                target,
                remote_port,
                ..
            } => (vec![target], *remote_port),
            Self::Kubectl {
                context,
                namespace,
                resource,
                remote_port,
                ..
            } => (vec![context, namespace, resource], *remote_port),
            Self::Cloud { instance, .. } => (vec![instance], 1),
            Self::Cloudflared { hostname } => (vec![hostname], 1),
        }
    }
    fn validate_destination(&self) -> Result<(), HelperError> {
        match self {
            Self::Kubectl { resource, .. }
                if !["pod/", "svc/", "deployment/"].iter().any(|prefix| {
                    resource
                        .strip_prefix(prefix)
                        .is_some_and(|name| !name.is_empty())
                }) =>
            {
                Err(invalid("Use pod/name, svc/name or deployment/name"))
            }
            Self::Cloud { instance, .. }
                if instance.contains(['?', '&', '=', '#'])
                    || instance.chars().any(char::is_whitespace) =>
            {
                Err(invalid(
                    "Instance must not contain query options or whitespace",
                ))
            }
            Self::Cloudflared { hostname }
                if hostname.contains(['/', '?', '@', '#', ':'])
                    || hostname.chars().any(char::is_whitespace) =>
            {
                Err(invalid(
                    "Enter an Access hostname without a URL scheme, path or credentials",
                ))
            }
            Self::Custom { .. }
            | Self::Ssm { .. }
            | Self::Kubectl { .. }
            | Self::Cloud { .. }
            | Self::Cloudflared { .. } => Ok(()),
        }
    }
}
fn invalid_value(value: &str) -> bool {
    value.trim().is_empty() || value.starts_with('-') || value.chars().any(char::is_control)
}
fn invalid(message: &str) -> HelperError {
    HelperError::Invalid(message.into())
}
