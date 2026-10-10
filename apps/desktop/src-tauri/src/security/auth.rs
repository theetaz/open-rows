// SPDX-License-Identifier: Apache-2.0
//! Native authentication behind a testable boundary; no credential enters `OpenRows`.
use crate::connections::{IpcError, IpcErrorKind};

/// OS-owned authentication boundary.
pub(crate) trait Authenticator {
    /// Whether this machine can evaluate device-owner authentication.
    fn available(&self) -> bool;
    /// Authenticate the named action, refusing failure or cancellation.
    fn authenticate(&self, reason: &str) -> Result<(), IpcError>;
}

/// Production platform authenticator.
pub(crate) struct NativeAuthenticator;

#[cfg(not(target_os = "macos"))]
impl Authenticator for NativeAuthenticator {
    fn available(&self) -> bool {
        false
    }
    fn authenticate(&self, _reason: &str) -> Result<(), IpcError> {
        Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "OS authentication is unavailable on this platform",
        ))
    }
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "LocalAuthentication demands platform FFI; isolated to this implementation"
)]
impl Authenticator for NativeAuthenticator {
    fn available(&self) -> bool {
        use objc2_local_authentication::{LAContext, LAPolicy};
        // SAFETY: a newly owned context and a valid framework policy, no raw pointers.
        unsafe {
            LAContext::new()
                .canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthentication)
                .is_ok()
        }
    }

    fn authenticate(&self, reason: &str) -> Result<(), IpcError> {
        use objc2_foundation::NSString;
        use objc2_local_authentication::{LAContext, LAPolicy};
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let reply = block2::RcBlock::new(
            move |success: objc2::runtime::Bool, _error: *mut objc2_foundation::NSError| {
                let _ = sender.send(success.as_bool());
            },
        );
        // SAFETY: context is retained until the callback completes, the nonempty
        // reason is owned, and the callback captures only a thread-safe sender.
        let context = unsafe { LAContext::new() };
        unsafe {
            context
                .canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthentication)
                .map_err(|_| {
                    IpcError::new(
                        IpcErrorKind::Unsupported,
                        "OS authentication is unavailable",
                    )
                })?;
            context.evaluatePolicy_localizedReason_reply(
                LAPolicy::DeviceOwnerAuthentication,
                &NSString::from_str(reason),
                &reply,
            );
        }
        let success = receiver.recv_timeout(std::time::Duration::from_secs(120));
        // SAFETY: context is still owned; invalidation cancels a timed-out evaluation.
        unsafe { context.invalidate() };
        if success == Ok(true) {
            Ok(())
        } else {
            Err(IpcError::new(
                IpcErrorKind::Auth,
                "Authentication failed or was cancelled; nothing was run",
            ))
        }
    }
}

/// Authenticate an immutable checked execution off the async runtime.
pub(crate) async fn authenticate_checked(
    plan: openrows_core::safety::SafetyPlan,
    reason: String,
) -> Result<(), IpcError> {
    tauri::async_runtime::spawn_blocking(move || verify_plan(&NativeAuthenticator, &plan, &reason))
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "Authentication service stopped"))?
}

/// A plan is authenticated afresh, never by a caller-supplied receipt.
fn verify_plan(
    authenticator: &dyn Authenticator,
    plan: &openrows_core::safety::SafetyPlan,
    reason: &str,
) -> Result<(), IpcError> {
    if plan.gate() == openrows_core::safety::Gate::Block {
        return Err(IpcError::invalid("This connection is read-only"));
    }
    if plan.authentication_required && authenticator.available() {
        verify(authenticator, reason)?;
    }
    Ok(())
}

/// Execute authentication off the async runtime's worker threads.
pub(crate) async fn authenticate(reason: String) -> Result<(), IpcError> {
    tauri::async_runtime::spawn_blocking(move || verify(&NativeAuthenticator, &reason))
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "Authentication service stopped"))?
}

/// Invoke the OS boundary; unavailable machines refuse direct unlock requests.
fn verify(authenticator: &dyn Authenticator, reason: &str) -> Result<(), IpcError> {
    if !authenticator.available() {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "OS authentication is unavailable",
        ));
    }
    authenticator.authenticate(reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        available: bool,
        success: bool,
    }
    impl Authenticator for Fake {
        fn available(&self) -> bool {
            self.available
        }
        fn authenticate(&self, _reason: &str) -> Result<(), IpcError> {
            if self.success {
                Ok(())
            } else {
                Err(IpcError::new(IpcErrorKind::Auth, "Cancelled"))
            }
        }
    }
    #[test]
    fn failed_cancelled_or_unavailable_authentication_runs_nothing_and_cannot_unlock()
    -> Result<(), IpcError> {
        for (available, success) in [(true, false), (false, true)] {
            let fake = Fake { available, success };
            let mut lock = super::super::lock::Lock::default();
            lock.lock();
            let mut executions = 0;
            if verify(&fake, "execute reviewed statements").is_ok() {
                executions += 1;
            }
            if verify(&fake, "unlock OpenRows").is_ok() {
                lock.unlock(lock.generation(), std::time::Duration::ZERO)?;
            }
            assert_eq!(executions, 0);
            assert!(lock.locked);
        }
        let fake = Fake {
            available: true,
            success: true,
        };
        verify(&fake, "execute reviewed statements")?;
        let mut lock = super::super::lock::Lock::default();
        lock.lock();
        verify(&fake, "unlock OpenRows")?;
        lock.unlock(lock.generation(), std::time::Duration::ZERO)?;
        assert!(!lock.locked);
        Ok(())
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use openrows_core::safety::{SafetyPlan, SafetyPolicy};
    use openrows_sql::Dialect;
    use openrows_types::safety::Safety;
    struct Fake(std::cell::Cell<u32>);
    impl Authenticator for Fake {
        fn available(&self) -> bool {
            true
        }
        fn authenticate(&self, _reason: &str) -> Result<(), IpcError> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }
    #[test]
    fn authenticate_policies_prompt_for_the_exact_checked_run() -> Result<(), IpcError> {
        for mode in [Safety::AuthenticateAll, Safety::AuthenticateWrites] {
            for sql in ["SELECT 1", "UPDATE t SET a=1 WHERE id=1", "BEGIN"] {
                let plan = SafetyPlan::check(
                    SafetyPolicy {
                        mode,
                        dialect: Dialect::Postgres,
                    },
                    vec![sql.into()],
                );
                let fake = Fake(std::cell::Cell::new(0));
                verify_plan(&fake, &plan, "execute reviewed statements")?;
                assert_eq!(fake.0.get(), u32::from(plan.authentication_required));
            }
        }
        assert_eq!(Safety::AuthenticateAll.fallback(), Safety::ConfirmAll);
        assert_eq!(Safety::AuthenticateWrites.fallback(), Safety::ConfirmWrites);
        Ok(())
    }
}
