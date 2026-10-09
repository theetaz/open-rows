// SPDX-License-Identifier: Apache-2.0
//! Integration tests against a real `PostgreSQL` server in a throwaway container.
//!
//! Ignored by `just test`; `just test-pg [version]` starts the container, runs
//! them with `--run-ignored only` and removes it (skipping when Docker is absent).
mod catalog;
mod connect;
mod diagram;
mod dump;
mod objects;
mod streaming;
mod structure;
mod support;
mod tls;
mod types;
