// SPDX-License-Identifier: Apache-2.0
//! `ClickHouse` error numbers mapped to the shared SQLSTATE contract.
use openrows_driver_api::{ConnectRequest, DriverError};

pub(crate) fn server(header: Option<&str>, body: &str, config: &ConnectRequest) -> DriverError {
    let number = header
        .and_then(|value| value.parse::<u32>().ok())
        .or_else(|| {
            body.strip_prefix("Code: ")?
                .split(['.', ','])
                .next()?
                .parse()
                .ok()
        })
        .unwrap_or(0);
    let mut message = body.to_owned();
    if let Some(password) = &config.password {
        let value = password.expose();
        if !value.is_empty() {
            message = message.replace(value, "[redacted]");
            let encoded: String = url::form_urlencoded::byte_serialize(value.as_bytes()).collect();
            message = message.replace(&encoded, "[redacted]");
        }
    }
    if matches!(number, 192 | 193 | 194 | 195 | 516) {
        return DriverError::Auth(message);
    }
    DriverError::Server {
        code: code(number).to_owned(),
        message: format!("{message} (ClickHouse error {number})"),
    }
}

fn code(number: u32) -> &'static str {
    match number {
        62 => "42601",
        60 | 81 => "42P01",
        16 | 47 => "42703",
        164 => "25006",
        497 => "42501",
        236 | 285 | 394 | 499 => "57014",
        349 => "23502",
        70 => "22018",
        373 => "55P03",
        _ => "HY000",
    }
}

pub(crate) fn cancelled() -> DriverError {
    DriverError::Server {
        code: "57014".to_owned(),
        message: "ClickHouse query cancelled".to_owned(),
    }
}
