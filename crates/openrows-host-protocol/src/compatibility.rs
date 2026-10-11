// SPDX-License-Identifier: Apache-2.0
//! Report contract differences without logging peer-controlled strings or payloads.
use crate::{Hello, HostDriverId};

pub(crate) fn report(observed: &Hello, expected: &Hello) {
    macro_rules! numeric {
        ($($field:ident),*) => { $(
            if observed.$field != expected.$field {
                tracing::warn!(check = concat!("hello_", stringify!($field)),
                    observed = ?observed.$field, expected = ?expected.$field,
                    "driver host compatibility check failed");
            }
        )* };
    }
    numeric!(version, generation, limits, arrow, required);
    for (check, equal) in [
        ("hello_schema", observed.schema == expected.schema),
        ("hello_build", observed.build == expected.build),
        (
            "hello_capabilities",
            observed.capabilities == expected.capabilities,
        ),
    ] {
        if !equal {
            tracing::warn!(check, "driver host compatibility check failed");
        }
    }
    if observed.driver != expected.driver {
        tracing::warn!(check = "hello_driver",
            observed = ?HostDriverId::parse(&observed.driver),
            expected = ?HostDriverId::parse(&expected.driver),
            "driver host compatibility check failed");
    }
}
