// SPDX-License-Identifier: Apache-2.0
//! Arrow field-metadata keys and extension names from docs/04 §5.

/// Arrow's standard extension-name key.
pub const EXTENSION_NAME: &str = "ARROW:extension:name";
/// Source database type name, carried by every result field.
pub const SOURCE_TYPE: &str = "openrows.source_type";
/// Canonical Arrow UUID extension over `FixedSizeBinary(16)`.
pub const UUID_EXTENSION: &str = "arrow.uuid";
/// JSON text stored as `Utf8`.
pub const JSON_EXTENSION: &str = "openrows.json";
/// Exact decimal text for values that do not fit `Decimal128`.
pub const DECIMAL_EXTENSION: &str = "openrows.decimal";
/// `"true"` on a temporal or decimal field that holds the server's text form
/// because some value had no exact Arrow representation (e.g. `infinity`);
/// [`SOURCE_TYPE`] still names the original type (docs/04 §5, docs/05 §11).
pub const TEXT_FALLBACK: &str = "openrows.text_fallback";
