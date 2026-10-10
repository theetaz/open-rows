// SPDX-License-Identifier: Apache-2.0
//! Hrana 3 JSON structures; tokens belong only in the HTTP authorization header.
mod request;
pub(crate) use request::{Batch, Pipeline, Request, Statement};
pub use request::{NamedArg, Value};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Error {
    pub(crate) message: String,
    pub(crate) code: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Reply {
    pub(crate) baton: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) results: Vec<StreamResult>,
}

impl Reply {
    pub(crate) fn validate(
        &self,
        requests: &[Request],
    ) -> Result<(), openrows_driver_api::DriverError> {
        if self.results.len() != requests.len()
            || (self.baton.is_none()
                && requests
                    .iter()
                    .any(|request| !matches!(request, Request::Close)))
            || self
                .results
                .iter()
                .zip(requests)
                .any(|(result, request)| !result.matches(request))
        {
            return Err(crate::error::protocol());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum StreamResult {
    Ok { response: Response },
    Error { error: Error },
}

impl StreamResult {
    fn matches(&self, request: &Request) -> bool {
        match self {
            Self::Error { .. } => !matches!(request, Request::GetAutocommit),
            Self::Ok { response } => matches!(
                (request, response),
                (Request::Execute { .. }, Response::Execute { .. })
                    | (Request::Batch { .. }, Response::Batch { .. })
                    | (Request::Sequence { .. }, Response::Sequence)
                    | (Request::Close, Response::Close)
                    | (Request::GetAutocommit, Response::GetAutocommit { .. })
            ),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Response {
    Execute { result: StatementResult },
    Batch { result: BatchResult },
    Sequence,
    Close,
    GetAutocommit { is_autocommit: bool },
}

#[derive(Default, Deserialize)]
pub(crate) struct StatementResult {
    pub(crate) affected_row_count: u64,
}

#[derive(Deserialize)]
pub(crate) struct BatchResult {
    pub(crate) step_results: Vec<Option<StatementResult>>,
    pub(crate) step_errors: Vec<Option<Error>>,
}
