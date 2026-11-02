// SPDX-License-Identifier: Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &str| {
    let _ = openrows_sql::mql::parse(data);
    let _ = openrows_sql::mql::classify(data);
});
