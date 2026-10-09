// SPDX-License-Identifier: Apache-2.0
//! Projects (FR-WS-03, FR-EDIT-08, FR-EDIT-14): a folder with
//! `.openrows/project.yaml` whose saved queries are plain `.sql` files in its
//! query folders, so a team can keep them in git. [`manifest`] reads and
//! writes the YAML, [`paths`] keeps every path inside the project, [`tree`]
//! lists the query folders and [`ops`] creates, renames, moves, copies and
//! deletes their items. Query files open in tabs through the `.sql` file
//! binding of `queries::files`.
pub(crate) mod commands;
mod error;
mod manifest;
mod ops;
mod paths;
mod tree;
