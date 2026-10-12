// SPDX-License-Identifier: Apache-2.0
use crate::error;
use mongodb::{
    Client,
    bson::doc,
    event::{EventHandler, command::CommandEvent},
    options::{SelectionCriteria, ServerAddress},
};
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Clone)]
struct Active {
    token: String,
    address: Option<ServerAddress>,
    index_build: bool,
}
#[derive(Default)]
pub(crate) struct State {
    current: Mutex<Option<Active>>,
}
impl State {
    pub(crate) fn handler(state: Arc<Self>) -> EventHandler<CommandEvent> {
        EventHandler::callback(move |event| {
            if let CommandEvent::Started(event) = event
                && let Ok(token) = event.command.get_str("comment")
            {
                state.started(token, event.connection.address);
            }
        })
    }
    fn started(&self, token: &str, address: ServerAddress) {
        if let Some(active) = self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
            .filter(|active| active.token == token)
        {
            active.address = Some(address);
        }
    }
}
pub(crate) struct Cancel {
    client: Client,
    state: Arc<State>,
    pub(crate) signal: watch::Sender<u64>,
}
impl Cancel {
    pub(crate) fn new(client: Client, state: Arc<State>) -> Arc<Self> {
        Arc::new(Self {
            client,
            state,
            signal: watch::channel(0).0,
        })
    }
    pub(crate) fn set(&self, token: Option<String>, index_build: bool) {
        *self
            .state
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = token.map(|token| Active {
            token,
            address: None,
            index_build,
        });
    }
}
impl CancelHandle for Cancel {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let token = self
            .state
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|active| active.token.clone());
        let state = self.state.clone();
        let client = self.client.clone();
        let signal = self.signal.clone();
        Box::pin(async move {
            let Some(token) = token else {
                return Ok(());
            };
            let Some(active) = signal_active(&state, &token, &signal) else {
                return Ok(());
            };
            tokio::time::timeout(std::time::Duration::from_secs(5), stop(client, active))
                .await
                .map_err(|_| DriverError::Network(std::io::ErrorKind::TimedOut.into()))?
        })
    }
}
fn signal_active(state: &State, token: &str, signal: &watch::Sender<u64>) -> Option<Active> {
    let current = state
        .current
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let active = current
        .as_ref()
        .filter(|active| active.token == token)?
        .clone();
    signal.send_modify(|generation| *generation = generation.wrapping_add(1));
    Some(active)
}
impl Active {
    fn selection(&self) -> Option<SelectionCriteria> {
        // Index builds continue server-side: killOp is not a safe way to abort them.
        if self.index_build {
            return None;
        }
        let address = self.address.clone()?;
        Some(SelectionCriteria::Predicate(Arc::new(move |server| {
            server.address() == &address
        })))
    }
}
async fn stop(client: Client, active: Active) -> Result<(), DriverError> {
    let Some(selection) = active.selection() else {
        return Ok(());
    };
    let command = doc! {
        "aggregate": 1,
        "pipeline": [
            {"$currentOp": {"allUsers": false, "localOps": true}},
            {"$match": {"command.comment": &active.token}},
        ],
        "cursor": {},
    };
    let mut operations = client
        .database("admin")
        .run_cursor_command(command)
        .selection_criteria(selection.clone())
        .await
        .map_err(error::mongo)?;
    while operations.advance().await.map_err(error::mongo)? {
        let operation = operations.deserialize_current().map_err(error::mongo)?;
        if operation
            .get_document("command")
            .is_ok_and(|command| command.contains_key("createIndexes"))
        {
            continue;
        }
        if let Some(id) = operation.get("opid") {
            client
                .database("admin")
                .run_command(doc! {"killOp":1,"op":id.clone()})
                .selection_criteria(selection.clone())
                .await
                .map_err(error::mongo)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_command_events_cannot_redirect_a_new_operation() {
        let state = State {
            current: Mutex::new(Some(Active {
                token: "new".into(),
                address: None,
                index_build: false,
            })),
        };
        let address = ServerAddress::Tcp {
            host: "localhost".into(),
            port: Some(27017),
        };
        state.started("old", address.clone());
        let (signal, _) = watch::channel(0);
        assert!(signal_active(&state, "old", &signal).is_none());
        assert_eq!(*signal.borrow(), 0);
        assert!(
            state
                .current
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|active| active.address.is_none())
        );
        state.started("new", address);
        assert!(
            state
                .current
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|active| active.address.is_some())
        );
    }
    #[test]
    fn index_cancellation_never_issues_kill_op() {
        let active = Active {
            token: "index".into(),
            address: Some(ServerAddress::Tcp {
                host: "localhost".into(),
                port: Some(27017),
            }),
            index_build: true,
        };
        assert!(active.selection().is_none());
    }
}
