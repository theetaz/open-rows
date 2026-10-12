// SPDX-License-Identifier: Apache-2.0
use openrows_driver_api::DriverError;

pub(crate) fn mongo(error: mongodb::error::Error) -> DriverError {
    use mongodb::error::ErrorKind;
    match *error.kind {
        ErrorKind::Authentication { .. } => {
            DriverError::Auth("MongoDB rejected the credentials".into())
        }
        ErrorKind::ServerSelection { .. } => DriverError::Network(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "MongoDB server selection failed; check the route, host and TLS settings",
        )),
        ErrorKind::InvalidArgument { .. } => invalid(),
        ErrorKind::Io(error) => DriverError::Network(std::io::Error::new(
            error.kind(),
            "MongoDB transport failed",
        )),
        ErrorKind::Command(command) => DriverError::Server {
            code: match command.code {
                50 | 11601 | 237 => "57014",
                112 | 251 => "40001",
                _ => "HY000",
            }
            .into(),
            message: format!(
                "MongoDB command failed (code {}, {})",
                command.code, command.code_name
            ),
        },
        _ => DriverError::Protocol(
            "MongoDB operation failed; check the connection and command".into(),
        ),
    }
}

pub(crate) fn invalid() -> DriverError {
    DriverError::Unsupported("invalid JSON/Extended JSON document or MQL arguments".into())
}

pub(crate) fn document(value: &serde_json::Value) -> Result<mongodb::bson::Document, DriverError> {
    validate_ejson(value)?;
    mongodb::bson::Bson::try_from(value.clone())
        .map_err(|_| invalid())?
        .as_document()
        .cloned()
        .ok_or_else(invalid)
}

fn validate_ejson(value: &serde_json::Value) -> Result<(), DriverError> {
    match value {
        serde_json::Value::Object(fields) => {
            let tags = [
                "$oid",
                "$symbol",
                "$regularExpression",
                "$numberInt",
                "$numberLong",
                "$numberDouble",
                "$numberDecimal",
                "$binary",
                "$uuid",
                "$timestamp",
                "$date",
                "$minKey",
                "$maxKey",
                "$dbPointer",
                "$undefined",
            ];
            if fields.len() != 1 && tags.iter().any(|tag| fields.contains_key(*tag)) {
                return Err(invalid());
            }
            if fields.contains_key("$code")
                && fields
                    .keys()
                    .any(|key| !matches!(key.as_str(), "$code" | "$scope"))
            {
                return Err(invalid());
            }
            for (tag, allowed) in [
                ("$binary", &["base64", "subType"][..]),
                ("$timestamp", &["t", "i"][..]),
                ("$regularExpression", &["pattern", "options"][..]),
                ("$dbPointer", &["$ref", "$id"][..]),
            ] {
                if fields
                    .get(tag)
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|inner| inner.keys().any(|key| !allowed.contains(&key.as_str())))
                {
                    return Err(invalid());
                }
            }
            fields.values().try_for_each(validate_ejson)
        }
        serde_json::Value::Array(values) => values.iter().try_for_each(validate_ejson),
        serde_json::Value::Number(number) if number.is_u64() && number.as_i64().is_none() => {
            Err(invalid())
        }
        _ => Ok(()),
    }
}
