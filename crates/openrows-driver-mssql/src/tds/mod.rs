// SPDX-License-Identifier: Apache-2.0
//! The Tabular Data Stream protocol, version 7.4 (MS-TDS): packets, the
//! `PRELOGIN`/TLS/`LOGIN7` handshake messages, requests and the response
//! tokens. Pure encoding and parsing except [`wire`], which moves packets.
pub(crate) mod collation;
pub(crate) mod cursor;
pub(crate) mod envchange;
pub(crate) mod login;
pub(crate) mod packet;
pub(crate) mod prelogin;
pub(crate) mod request;
pub(crate) mod tls;
pub(crate) mod token;
pub(crate) mod types;
pub(crate) mod wire;
