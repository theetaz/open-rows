// SPDX-License-Identifier: Apache-2.0
//! Sequential reads with monotonic cadence, bounded deadlines and generation ownership.
use super::Shared;
use crate::{Cadence, Descriptor, MonitorReader, Reading, SampleState, SourceStatus};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) struct Options {
    pub(super) shared: Arc<Mutex<Shared>>,
    pub(super) events: tokio::sync::broadcast::Sender<Vec<SourceStatus>>,
    pub(super) live_ms: u32,
}

struct Scheduled {
    descriptor: Descriptor,
    cadence: Cadence,
    hash: u64,
}

pub(super) async fn run(
    mut reader: Box<dyn MonitorReader>,
    descriptors: Vec<Descriptor>,
    options: Options,
) {
    let Options {
        shared,
        events,
        live_ms,
    } = options;
    let start = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .started;
    let mut last_event = None;
    let mut sources: Vec<_> = descriptors
        .into_iter()
        .map(|descriptor| Scheduled {
            cadence: Cadence::new(descriptor.source.floor_ms(live_ms)),
            descriptor,
            hash: 0,
        })
        .collect();
    // Completion-based floors must not accumulate a whole extra second each read.
    let mut timer = tokio::time::interval(Duration::from_millis(100));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let (mut minute, mut spent) = (0, 0);
    'collect: loop {
        timer.tick().await;
        let now = milliseconds(start.elapsed());
        if now / 60_000 != minute {
            minute = now / 60_000;
            spent = 0;
        }
        for scheduled in &mut sources {
            if !scheduled.cadence.due(now) {
                continue;
            }
            if spent >= 12_000 {
                scheduled.cadence.throttle(now);
                continue;
            }
            let began = tokio::time::Instant::now();
            let reading = read(&mut *reader, scheduled).await;
            let terminal = reading.terminal;
            let duration = milliseconds(began.elapsed());
            spent += duration;
            publish(
                &shared,
                scheduled,
                reading,
                (milliseconds(start.elapsed()), duration),
            );
            if terminal {
                let _ = tokio::time::timeout(Duration::from_secs(1), reader.cancellation()).await;
                let _ = events.send(
                    shared
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .store
                        .snapshot(),
                );
                break 'collect;
            }
            if spent > 12_000 {
                scheduled.cadence.throttle(milliseconds(start.elapsed()));
            }
        }
        let snapshot = shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .store
            .snapshot();
        if last_event
            .is_none_or(|last: tokio::time::Instant| last.elapsed() >= Duration::from_secs(1))
        {
            let _ = events.send(snapshot);
            last_event = Some(tokio::time::Instant::now());
        }
    }
}

async fn read(reader: &mut dyn MonitorReader, source: &Scheduled) -> Reading {
    if source.descriptor.state != SampleState::Ok {
        return Reading::missing(source.descriptor.state.clone());
    }
    if let Ok(reading) = tokio::time::timeout(
        Duration::from_millis(u64::from(source.cadence.deadline_ms())),
        reader.read(source.descriptor.source, source.cadence.deadline_ms()),
    )
    .await
    {
        reading
    } else {
        // Cancelling a protocol read invalidates the session.
        let mut reading = Reading::missing(SampleState::Gap);
        reading.terminal = true;
        reading
    }
}

fn publish(
    shared: &Mutex<Shared>,
    scheduled: &mut Scheduled,
    reading: Reading,
    (elapsed, duration): (u64, u64),
) {
    let mut shared = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if shared.stopped {
        return;
    }
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1_000.0;
    let mut hasher = DefaultHasher::new();
    reading.rows.hash(&mut hasher);
    let hash = hasher.finish();
    let failed = reading.state != SampleState::Ok;
    scheduled.cadence.complete(
        elapsed,
        hash != scheduled.hash,
        failed,
        u32::try_from(duration).unwrap_or(u32::MAX),
    );
    scheduled.hash = hash;
    let source = scheduled.descriptor.source;
    let old = shared.store.status.get(&source);
    let status = SourceStatus {
        source,
        state: reading.state.clone(),
        collected_ms: wall,
        last_success_ms: if failed {
            old.and_then(|s| s.last_success_ms)
        } else {
            Some(wall)
        },
        interval_ms: scheduled.cadence.interval_ms(),
        duration_ms: Duration::from_millis(duration).as_secs_f64() * 1_000.0,
        sequence: old.map_or(1, |s| s.sequence.saturating_add(1)),
        partial: reading.partial,
    };
    shared.store.status.insert(source, status);
    shared.store.ingest(source, reading, elapsed, wall);
}

fn milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
