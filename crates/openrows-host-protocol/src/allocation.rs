// SPDX-License-Identifier: Apache-2.0
//! A synchronous decode scope limits aggregate nested collection allocations.
use std::cell::Cell;
const COLLECTION_BYTES: usize = 32 * 1024 * 1024;
thread_local! {
    static REMAINING: Cell<Option<usize>> = const { Cell::new(None) };
}
pub(crate) struct Scope(Option<usize>);
impl Scope {
    pub(crate) fn begin() -> Self {
        Self(REMAINING.replace(Some(COLLECTION_BYTES)))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        REMAINING.set(self.0);
    }
}
pub(crate) fn charge<T>(items: usize) -> Result<(), ()> {
    // Vec growth can reserve almost twice its logical item count. String data
    // is separately bounded by the encoded frame/facet length.
    let bytes = items
        .checked_mul(size_of::<T>())
        .and_then(|n| n.checked_mul(2))
        .ok_or(())?;
    REMAINING.with(|remaining| {
        if let Some(current) = remaining.get() {
            remaining.set(Some(current.checked_sub(bytes).ok_or(())?));
        }
        Ok(())
    })
}
