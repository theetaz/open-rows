// SPDX-License-Identifier: Apache-2.0
//! HTTP request construction: credentials only in sensitive headers.
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, header::HeaderValue};
use openrows_driver_api::{ConnectRequest, DriverError};

pub(crate) fn build(
    config: &ConnectRequest,
    sql: &str,
    params: &[Option<String>],
    identity: (&str, Option<&str>),
) -> Result<Request<Full<Bytes>>, DriverError> {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("query_id", identity.0);
    query.append_pair("wait_end_of_query", "0");
    query.append_pair("cancel_http_readonly_queries_on_client_close", "1");
    query.append_pair("output_format_arrow_use_signed_indexes_for_dictionary", "1");
    query.append_pair("default_format", "ArrowStream");
    query.append_pair("output_format_arrow_compression_method", "none");
    query.append_pair("max_block_size", "200");
    query.append_pair("preferred_block_size_bytes", "4194304");
    query.append_pair("output_format_arrow_string_as_string", "1");
    query.append_pair("output_format_arrow_low_cardinality_as_dictionary", "1");
    if let Some(session) = identity.1 {
        query.append_pair("session_id", session);
    }
    for (index, value) in params.iter().enumerate() {
        query.append_pair(
            &format!("param_p{}", index + 1),
            &parameter(value.as_deref()),
        );
    }
    let mut request = Request::post(format!("/?{}", query.finish()))
        .body(Full::new(Bytes::copy_from_slice(sql.as_bytes())))
        .map_err(|_| DriverError::Protocol("invalid HTTP request".to_owned()))?;
    let headers = request.headers_mut();
    headers.insert("host", header(&config.describe_endpoint(), false)?);
    headers.insert("x-clickhouse-user", header(&config.user, false)?);
    if let Some(password) = &config.password {
        headers.insert("x-clickhouse-key", header(password.expose(), true)?);
    }
    headers.insert("x-clickhouse-database", header(&config.database, false)?);
    Ok(request)
}

fn header(value: &str, sensitive: bool) -> Result<HeaderValue, DriverError> {
    let mut header = HeaderValue::from_str(value).map_err(|_| {
        DriverError::Protocol("invalid ClickHouse credential or database header".to_owned())
    })?;
    header.set_sensitive(sensitive);
    Ok(header)
}

/// `ClickHouse` HTTP parameters use the escaped input format, including `\N` for NULL.
fn parameter(value: Option<&str>) -> String {
    value.map_or_else(
        || "\\N".to_owned(),
        |text| {
            text.replace('\\', "\\\\")
                .replace('\0', "\\0")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t")
        },
    )
}
