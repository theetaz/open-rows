// SPDX-License-Identifier: Apache-2.0
//! Suspended sessions (M1-01, docs/04 §3.2): after the profile's idle
//! timeout a resilient session closes its connection and waits. The next
//! statement reconnects through the session's [`Link`](crate::resilience)
//! (route, setup statements and replayed settings, with backoff) and then
//! runs; `Close` while suspended only ends the actor.
use crate::actor::{Actor, Mailbox, fail_waiting};
use crate::resilience::Recovery;
use crate::shared::{Command, Queued};
use crate::{CoreError, QueryEvent, SessionState};

/// Close `actor`'s connection and wait for the next statement; `Some` is
/// the actor on a new connection with that statement pending, `None` means
/// the session ended (closed, every sender gone, or reconnecting failed).
pub(crate) async fn sleep(actor: Actor) -> Option<Actor> {
    let mut mail = actor.disconnect().await;
    mail.shared.set_state(SessionState::Suspended, None);
    tracing::info!(session = %mail.shared.id, "idle session suspended");
    loop {
        match mail.commands.recv().await {
            Some(Command::Execute(queued)) if queued.token.is_cancelled() => {
                let _ = queued.events.send(QueryEvent::Cancelled);
            }
            Some(Command::Execute(queued)) => return wake(mail, queued).await,
            Some(Command::Suspend { .. }) => {}
            Some(Command::Close { reply }) => {
                mail.shared.set_state(SessionState::Closed, None);
                // `close` may have stopped waiting; the state says Closed.
                let _ = reply.send(Ok(()));
                return None;
            }
            None => {
                mail.shared.set_state(SessionState::Closed, None);
                return None;
            }
        }
    }
}

/// Reconnect for `queued`; it fails with the reason when that is impossible.
async fn wake(mut mail: Mailbox, queued: Queued) -> Option<Actor> {
    mail.shared.set_state(SessionState::Connecting, None);
    let recovery = match mail.link.as_ref() {
        Some(link) => link.restore(&mail.shared.token).await,
        None => Recovery::Failed(CoreError::session_closed(mail.shared.id)),
    };
    match recovery {
        Recovery::Restored(session) => {
            if let Some(link) = mail.link.as_mut() {
                link.touch();
            }
            mail.shared.set_state(SessionState::Idle, None);
            Some(Actor::new(mail, session, Some(queued)))
        }
        Recovery::Closed => {
            let error = CoreError::session_closed(mail.shared.id);
            let _ = queued.events.send(QueryEvent::Failed { error });
            mail.shared.set_state(SessionState::Closed, None);
            None
        }
        Recovery::Failed(error) => {
            let _ = queued.events.send(QueryEvent::Failed {
                error: error.clone(),
            });
            mail.shared.fail(error.clone());
            fail_waiting(&mut mail, &error);
            None
        }
    }
}
