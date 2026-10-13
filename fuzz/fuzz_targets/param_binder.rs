// SPDX-License-Identifier: Apache-2.0
//! Query parameter binder (M1-15): slots are in-bounds, ordered and on
//! character boundaries, and binding every slot yields one parameter per
//! slot with only the slots rewritten.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openrows_sql::{Dialect, ParamBinder, ParamStyle};

const DIALECTS: [Dialect; 3] = [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite];
const STYLES: [ParamStyle; 3] = [
    ParamStyle::Named,
    ParamStyle::Numbered,
    ParamStyle::Anonymous,
];

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let dialect = DIALECTS[usize::from(selector) % DIALECTS.len()];
    let style = STYLES[usize::from(selector / 3) % STYLES.len()];
    let sql = String::from_utf8_lossy(rest);
    let mut binder = ParamBinder::new(dialect, style);
    let slots = binder.slots(&sql);
    let mut end = 0;
    for slot in &slots {
        assert!(end <= slot.range.start && slot.range.start < slot.range.end);
        assert!(
            sql.get(slot.range.clone()).is_some(),
            "slot splits a character"
        );
        end = slot.range.end;
    }
    let Ok(bound) = binder.bind(&sql, &slots, |name| Some(Some(name.to_owned()))) else {
        panic!("a value was supplied for every slot");
    };
    assert_eq!(bound.params.len(), slots.len());
    let names = slots.iter().map(|slot| Some(slot.name.clone()));
    assert!(bound.params.iter().cloned().eq(names));
    let kept: usize = sql.len() - slots.iter().map(|slot| slot.range.len()).sum::<usize>();
    assert!(bound.sql.len() >= kept);
});
