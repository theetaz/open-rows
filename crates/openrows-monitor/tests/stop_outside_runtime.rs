// SPDX-License-Identifier: Apache-2.0
//! The app's exit handler stops collectors on a thread without a Tokio runtime.
use openrows_monitor::{Descriptor, Monitor, MonitorReader, ReadFuture, Reading};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Idle {
    cancelled: Arc<AtomicUsize>,
}
impl MonitorReader for Idle {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(async { Vec::new() })
    }
    fn read(&mut self, _: openrows_monitor::Source, _: u32) -> ReadFuture<'_, Reading> {
        Box::pin(std::future::pending())
    }
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancelled = self.cancelled.clone();
        Box::pin(async move {
            cancelled.fetch_add(1, Ordering::SeqCst);
        })
    }
}

#[test]
fn stop_from_a_thread_without_a_runtime_still_sends_the_cancel()
-> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    let cancelled = Arc::new(AtomicUsize::new(0));
    let reader = Idle {
        cancelled: cancelled.clone(),
    };
    let monitor =
        runtime.block_on(async { Monitor::spawn(Box::new(reader), Vec::new(), 5_000) })?;
    // Like `RunEvent::Exit`: no runtime context on this thread.
    std::thread::spawn(move || monitor.stop())
        .join()
        .map_err(|_| "stop panicked outside the runtime")?;
    runtime.block_on(async {
        for _ in 0..100 {
            if cancelled.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
    });
    assert_eq!(cancelled.load(Ordering::SeqCst), 1);
    Ok(())
}
