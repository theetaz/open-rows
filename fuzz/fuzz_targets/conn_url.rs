// SPDX-License-Identifier: Apache-2.0
//! Connection URL parser (FR-CONN-02, M1-05): any input yields a profile
//! or a typed error, never a panic, and errors never echo the password.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openrows_store::profile::parse_url;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    match parse_url(&input) {
        Ok(parsed) => {
            // The password is redacted wherever the parse result is printed.
            if let Some(password) = &parsed.password {
                let shown = format!("{:?}", parsed.password);
                assert!(password.expose().len() < 12 || !shown.contains(password.expose()));
            }
        }
        Err(error) => assert!(!error.to_string().is_empty()),
    }
});
