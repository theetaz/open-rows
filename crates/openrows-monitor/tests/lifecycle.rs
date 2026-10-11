// SPDX-License-Identifier: Apache-2.0
//! A client deadline discards the protocol session and forbids follow-up reads.
use openrows_monitor::{
    Descriptor, Monitor, MonitorReader, ReadFuture, Reading, SampleState, Scope, Source,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

struct Hung {
    reads: Arc<AtomicUsize>,
    cancelled: Arc<AtomicUsize>,
    initial: Option<Reading>,
}
impl MonitorReader for Hung {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(async { Vec::new() })
    }
    fn read(&mut self, _: Source, _: u32) -> ReadFuture<'_, Reading> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(reading) = self.initial.take() {
            return Box::pin(async { reading });
        }
        Box::pin(std::future::pending())
    }
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancelled = self.cancelled.clone();
        Box::pin(async move {
            cancelled.fetch_add(1, Ordering::SeqCst);
        })
    }
}

#[tokio::test(start_paused = true)]
async fn client_deadline_cancels_and_never_reuses_an_undrained_session()
-> Result<(), Box<dyn std::error::Error>> {
    let (reads, cancelled) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let source = Descriptor {
        source: Source::Activity,
        state: SampleState::Ok,
        scope: Scope::Server,
        interval_ms: 5_000,
        guidance: None,
        coverage: "test".into(),
    };
    let monitor = Monitor::spawn(
        Box::new(Hung {
            reads: reads.clone(),
            cancelled: cancelled.clone(),
            initial: None,
        }),
        vec![source],
        5_000,
    )?;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    assert!(!monitor.reusable(5_000));
    assert_eq!(monitor.snapshot()[0].state, SampleState::Gap);
    tokio::time::advance(Duration::from_secs(120)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    monitor.stop();
    assert!(monitor.snapshot().is_empty());
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn reads_expire_history_after_a_terminal_gap() -> Result<(), Box<dyn std::error::Error>> {
    let source = Descriptor {
        source: Source::Activity,
        state: SampleState::Ok,
        scope: Scope::Server,
        interval_ms: 5_000,
        guidance: None,
        coverage: "test".into(),
    };
    let initial = Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: "a".into(),
        partial: false,
        columns: vec![
            openrows_monitor::Column {
                name: "pid".into(),
                unit: "id".into(),
                kind: openrows_monitor::MetricKind::Text,
            },
            openrows_monitor::Column {
                name: "state".into(),
                unit: "state".into(),
                kind: openrows_monitor::MetricKind::Text,
            },
        ],
        rows: vec![vec![Some("1".into()), Some("active".into())]],
    };
    let monitor = Monitor::spawn(
        Box::new(Hung {
            reads: Arc::new(AtomicUsize::new(0)),
            cancelled: Arc::new(AtomicUsize::new(0)),
            initial: Some(initial),
        }),
        vec![source],
        5_000,
    )?;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(monitor.activity().is_ok());
    tokio::time::advance(Duration::from_secs(5)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(!monitor.reusable(5_000));
    tokio::time::advance(Duration::from_secs(601)).await;
    let bytes = monitor.window(None, 1_000, false)?;
    let mut reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)?;
    assert_eq!(reader.next().ok_or("missing batch")??.num_rows(), 0);
    assert!(
        monitor
            .window(Some(Source::Activity), 1_000, false)
            .is_err()
    );
    Ok(())
}

struct FakeReader {
    active: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    cancelled: Arc<AtomicUsize>,
}
impl MonitorReader for FakeReader {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(async { vec![descriptor()] })
    }
    fn read(&mut self, _: Source, _: u32) -> ReadFuture<'_, Reading> {
        Box::pin(async move {
            assert_eq!(self.active.fetch_add(1, Ordering::SeqCst), 0);
            self.reads.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Reading::missing(SampleState::Ok)
        })
    }
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancelled = self.cancelled.clone();
        Box::pin(async move {
            cancelled.fetch_add(1, Ordering::SeqCst);
        })
    }
}
fn descriptor() -> Descriptor {
    Descriptor {
        source: Source::Database,
        state: SampleState::Ok,
        scope: Scope::Database,
        interval_ms: 15_000,
        guidance: None,
        coverage: "test".into(),
    }
}

#[tokio::test(start_paused = true)]
async fn one_actor_never_overlaps_and_stop_cancels_clears_and_rejects_late_reads()
-> Result<(), Box<dyn std::error::Error>> {
    let (active, reads, cancelled) = (
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let monitor = Monitor::spawn(
        Box::new(FakeReader {
            active,
            reads: reads.clone(),
            cancelled: cancelled.clone(),
        }),
        vec![descriptor()],
        5_000,
    )?;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(15)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    monitor.stop();
    tokio::time::advance(Duration::from_secs(60)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    assert!(monitor.snapshot().is_empty());
    assert!(monitor.window(None, 20, false).is_err());
    Ok(())
}
