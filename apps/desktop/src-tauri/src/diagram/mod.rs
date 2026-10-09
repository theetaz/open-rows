// SPDX-License-Identifier: Apache-2.0
//! The read-only ER diagram of a schema (FR-SCHEMA-06, M2-10).
//!
//! `schema_diagram` reads the schema's tables, columns and foreign keys on
//! the database's metadata session (bulk catalog queries on server engines),
//! capped at [`MAX_TABLES`] so huge schemas stay responsive. The webview lays
//! out and draws the diagram; `diagram_save` writes its SVG or PNG export to
//! a file chosen in the native save panel.
pub(crate) mod commands;
mod dto;

/// Most tables one diagram shows; the reply says when more exist.
pub(crate) const MAX_TABLES: usize = 400;
