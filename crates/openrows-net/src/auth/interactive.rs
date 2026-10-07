// SPDX-License-Identifier: Apache-2.0
//! Keyboard-interactive authentication (RFC 4256) via an injectable prompter,
//! so the UI can show MFA/OTP challenges as native dialogs later.

use russh::client::{Handle, KeyboardInteractiveAuthResponse as Reply};

use super::{AuthContext, AuthOutcome};
use crate::{BoxFuture, SecretString, SshError, handler::ClientHandler};

/// Upper bound on challenge rounds, so a misbehaving server cannot loop forever.
const MAX_ROUNDS: usize = 10;

/// One prompt in a challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengePrompt {
    /// Prompt text from the server, e.g. `Verification code: `.
    pub text: String,
    /// Whether the answer may be shown while typing.
    pub echo: bool,
}

/// A keyboard-interactive challenge from the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Challenge {
    /// Hop host the challenge comes from.
    pub host: String,
    /// Login user.
    pub user: String,
    /// Challenge name (often empty).
    pub name: String,
    /// Instructions to display (often empty).
    pub instructions: String,
    /// Prompts to answer, in order.
    pub prompts: Vec<ChallengePrompt>,
}

/// The user dismissed the challenge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("prompt cancelled by the user")]
pub struct PromptCancelled;

/// Answers keyboard-interactive challenges (UI dialog, test script, …).
///
/// Time spent here is excluded from the handshake timeout.
pub trait KeyboardInteractivePrompter: Send + Sync {
    /// Return one answer per prompt, in order.
    fn respond<'a>(
        &'a self,
        challenge: &'a Challenge,
    ) -> BoxFuture<'a, Result<Vec<SecretString>, PromptCancelled>>;
}

pub(crate) async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    context: &AuthContext<'_>,
) -> Result<AuthOutcome, SshError> {
    let mut reply = handle
        .authenticate_keyboard_interactive_start(context.user, None)
        .await?;
    for _ in 0..MAX_ROUNDS {
        let challenge = match reply {
            Reply::Success => return Ok(AuthOutcome::Success),
            Reply::Failure {
                partial_success, ..
            } => return Ok(AuthOutcome::from_partial(partial_success)),
            Reply::InfoRequest {
                name,
                instructions,
                prompts,
            } => Challenge {
                host: context.host.to_owned(),
                user: context.user.to_owned(),
                name,
                instructions,
                prompts: prompts
                    .into_iter()
                    .map(|prompt| ChallengePrompt {
                        text: prompt.prompt,
                        echo: prompt.echo,
                    })
                    .collect(),
            },
        };
        let answers = answer(&challenge, context).await?;
        // russh takes owned Strings; from here the copies are russh's to manage.
        let answers = answers
            .iter()
            .map(|answer| answer.expose().to_owned())
            .collect();
        reply = handle
            .authenticate_keyboard_interactive_respond(answers)
            .await?;
    }
    Ok(AuthOutcome::Rejected)
}

/// Ask the prompter (clock paused); empty challenges are answered silently.
async fn answer(
    challenge: &Challenge,
    context: &AuthContext<'_>,
) -> Result<Vec<SecretString>, SshError> {
    if challenge.prompts.is_empty() {
        return Ok(Vec::new());
    }
    let _pause = context.clock.pause();
    let answers = context
        .prompter
        .respond(challenge)
        .await
        .map_err(|PromptCancelled| SshError::PromptCancelled)?;
    if answers.len() == challenge.prompts.len() {
        Ok(answers)
    } else {
        Err(SshError::PromptCancelled)
    }
}
