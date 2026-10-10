// SPDX-License-Identifier: Apache-2.0
//! Quiet production entry point; native stdout/stderr are discarded by the parent.
mod entry;

fn main() {
    entry::run(openrows_driver_host::run());
}
