// SPDX-License-Identifier: Apache-2.0
//! The store's state behind its lock, and the spill pass over it.
use crate::cache::DecodedCache;
use crate::limits::{Append, Limits};
use crate::set::{ResultSet, Slot};
use crate::slice::ranges;
use crate::spill::{Spill, SpillFile, Victim, pick};
use crate::store::{Window, WindowError};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{
    Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak,
};

/// The batches (shared, not copied) and row ranges one window encodes.
pub(crate) struct WindowPlan {
    pub(crate) schema: SchemaRef,
    pub(crate) indices: Vec<usize>,
    pub(crate) slices: Vec<(Slot, Range<usize>)>,
}

#[derive(Default)]
pub(crate) struct Inner {
    pub(crate) results: HashMap<String, ResultSet>,
    /// Bytes of every result's batches still in RAM.
    pub(crate) bytes: usize,
    generations: u64,
}

impl Inner {
    /// Remove a result, returning it so the caller drops it (and deletes
    /// its spill files) after releasing the lock.
    pub(crate) fn take(&mut self, id: &str) -> Option<ResultSet> {
        let old = self.results.remove(id)?;
        self.bytes = self.bytes.saturating_sub(old.bytes);
        Some(old)
    }

    /// Store `result` under `id`, returning the result it replaces.
    pub(crate) fn put(&mut self, id: String, mut result: ResultSet) -> Option<ResultSet> {
        let old = self.take(&id);
        self.generations += 1;
        result.generation = self.generations;
        self.bytes = self.bytes.saturating_add(result.bytes);
        self.results.insert(id, result);
        old
    }

    /// Point a victim's slots at `file`, unless its result was released or
    /// replaced while the file was written.
    fn swap_in(&mut self, victim: &Victim, file: &Arc<SpillFile>) {
        let Some(result) = self.results.get_mut(&victim.id) else {
            return;
        };
        if result.generation != victim.generation {
            return;
        }
        for (position, (index, _)) in victim.batches.iter().enumerate() {
            let spilled = Slot::Spilled {
                file: Arc::clone(file),
                index: position,
            };
            let freed = result.spill_slot(*index, spilled);
            self.bytes = self.bytes.saturating_sub(freed);
        }
    }

    pub(crate) fn get(&self, id: &str) -> Result<&ResultSet, WindowError> {
        self.results
            .get(id)
            .ok_or_else(|| WindowError::UnknownResult(id.to_owned()))
    }
}

/// State shared by the store handle, the spill thread and export readers.
pub(crate) struct Shared {
    inner: RwLock<Inner>,
    pub(crate) changed: tokio::sync::Notify,
    pub(crate) pressured: AtomicBool,
    pub(crate) limits: Limits,
    /// Logical time for least-recently-used eviction.
    clock: AtomicU64,
    cache: Mutex<DecodedCache>,
    pub(crate) spill: Option<Spill>,
}

/// The spill thread's pass; holds the store weakly so it can be dropped.
pub(crate) fn spill_pass(store: Weak<Shared>) -> impl Fn() + Send + 'static {
    move || {
        if let Some(shared) = store.upgrade() {
            shared.spill();
        }
    }
}

impl Shared {
    pub(crate) fn new(limits: Limits, spill: Option<Spill>) -> Self {
        Self {
            inner: RwLock::default(),
            changed: tokio::sync::Notify::new(),
            pressured: AtomicBool::new(false),
            limits,
            clock: AtomicU64::new(0),
            cache: Mutex::default(),
            spill,
        }
    }

    pub(crate) fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn can_spill(&self) -> bool {
        self.spill.as_ref().is_some_and(Spill::available)
    }

    pub(crate) fn request_spill(&self) {
        if let Some(spill) = self.spill.as_ref().filter(|spill| spill.available()) {
            spill.request();
        }
    }

    pub(crate) fn spill_if_over(&self) {
        if self.read().bytes > self.limits.max_bytes {
            self.request_spill();
        }
    }

    /// Store `batch`; returns the flow and whether memory is over budget.
    pub(crate) fn append(&self, id: &str, batch: RecordBatch) -> (Append, bool) {
        let can_spill = self.can_spill();
        let now = self.tick();
        let mut guard = self.write();
        let inner = &mut *guard;
        let Some(result) = inner.results.get_mut(id) else {
            return (Append::Full, false);
        };
        result.touch(now);
        if batch.schema() != result.schema {
            result.truncated = true;
            return (Append::Full, false);
        }
        let room = self.limits.max_rows.saturating_sub(result.rows);
        let batch = if batch.num_rows() > room {
            result.truncated = true;
            batch.slice(0, room)
        } else {
            batch
        };
        let bytes = batch.get_array_memory_size();
        let over = inner.bytes.saturating_add(bytes) > self.limits.max_bytes;
        if over && !can_spill {
            result.truncated = true;
            return (Append::Full, false);
        }
        inner.bytes += result.push(batch, bytes);
        let flow = if result.truncated {
            Append::Full
        } else {
            Append::Continue
        };
        (flow, over)
    }

    /// Projected schema, column indices and the slots and row ranges of a
    /// window, collected under the read lock; decoding and encoding happen
    /// after it is released.
    pub(crate) fn plan(&self, id: &str, window: Window) -> Result<WindowPlan, WindowError> {
        let now = self.tick();
        let inner = self.read();
        let result = inner.get(id)?;
        result.touch(now);
        let (rows, cols) = ranges(result.rows, result.schema.fields().len(), window)?;
        let indices: Vec<_> = cols.collect();
        let schema = Arc::new(result.schema.project(&indices)?);
        Ok(WindowPlan {
            schema,
            indices,
            slices: result.slices(&rows),
        })
    }

    /// The batch of a slot, decoding a spilled one through the cache.
    pub(crate) fn load(&self, slot: Slot) -> Result<RecordBatch, WindowError> {
        match slot {
            Slot::Memory { batch, .. } => Ok(batch),
            Slot::Spilled { file, index } => {
                let key = (file.id(), index);
                if let Some(batch) = self.cache().get(key) {
                    return Ok(batch);
                }
                let batch = file.read(index)?;
                self.cache().put(key, batch.clone());
                Ok(batch)
            }
        }
    }

    /// Slot `index` of result `id` as stored under `generation`.
    pub(crate) fn slot(
        &self,
        (id, generation): (&str, u64),
        index: usize,
    ) -> Result<Slot, WindowError> {
        let inner = self.read();
        inner
            .results
            .get(id)
            .filter(|result| result.generation == generation)
            .and_then(|result| result.slot(index).cloned())
            .ok_or_else(|| WindowError::UnknownResult(id.to_owned()))
    }

    /// One spill pass (runs on the spill thread): pick, write, swap in.
    fn spill(&self) {
        let Some(spill) = &self.spill else {
            return;
        };
        let budget = self.limits.max_bytes;
        let victims = {
            let inner = self.read();
            if inner.bytes <= budget {
                return;
            }
            // Spill to three quarters of the budget so the next appends do
            // not trigger another pass straight away.
            pick(&inner.results, inner.bytes - (budget - budget / 4))
        };
        let mut written = Vec::new();
        for victim in victims {
            match spill.write(&victim) {
                Ok(file) => written.push((victim, file)),
                Err(_) => break,
            }
        }
        let mut inner = self.write();
        for (victim, file) in &written {
            inner.swap_in(victim, file);
        }
        drop(inner);
        self.changed.notify_waiters();
        // `written` drops here, outside the lock: files of results released
        // during the write are deleted then.
    }

    pub(crate) fn cache(&self) -> MutexGuard<'_, DecodedCache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }
}
