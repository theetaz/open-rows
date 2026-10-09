// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Processes`]: list, cancel and terminate another
//! connection's process from a listing connection.

use std::time::{Duration, Instant};

use openrows_driver_api::{Connection, DriverError, ProcessId, ProcessSignal, admin};

use crate::dialect::{cancels_other_sessions, codes, sleep};
use crate::sink::{Collect, ping, sqlstate};
use crate::{Failure, Target, ensure};

/// The sleeper sleeps this long unless cancelled.
const SLEEP_SECONDS: u32 = 60;

/// A cancelled or terminated process must be gone well before this.
const DEADLINE: Duration = Duration::from_secs(15);

/// The listing polls at this interval while waiting for a change.
const POLL: Duration = Duration::from_millis(50);

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let mut lister = target.connect().await?;
    let mut sleeper = target.connect().await?;
    lists_itself(lister.as_mut()).await?;
    let sleeper_id = admin(sleeper.as_mut())?.own_process().await?;
    if cancels_other_sessions(target.driver.dialect()) {
        cancel_sleeper(target, lister.as_mut(), sleeper.as_mut(), sleeper_id).await?;
    } else {
        cannot_cancel(lister.as_mut(), sleeper_id).await?;
    }
    terminate(lister.as_mut(), sleeper.as_mut(), sleeper_id).await?;
    lister.close().await?;
    // The sleeper's server process is gone; closing only drops the socket.
    let _ = sleeper.close().await;
    Ok(())
}

/// The listing connection is listed once, as current, under its own id.
async fn lists_itself(lister: &mut dyn Connection) -> Result<(), Failure> {
    let facet = admin(lister)?;
    let own = facet.own_process().await?;
    let processes = facet.list_processes().await?;
    let current: Vec<ProcessId> = processes
        .iter()
        .filter(|process| process.current)
        .map(|process| process.id)
        .collect();
    ensure(current == [own], || {
        format!("own process {own}, listed as current: {current:?}")
    })
}

/// A sleeping statement is listed with its text; cancelling it ends it
/// with the cancel SQLSTATE and leaves the session usable.
async fn cancel_sleeper(
    target: &Target,
    lister: &mut dyn Connection,
    sleeper: &mut dyn Connection,
    sleeper_id: ProcessId,
) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let sql = sleep(dialect, SLEEP_SECONDS);
    let started = Instant::now();
    let mut sink = Collect::default();
    let (slept, cancelled) = tokio::join!(
        sleeper.execute(&sql, &mut sink),
        cancel_when_listed(lister, sleeper_id),
    );
    cancelled?;
    let elapsed = started.elapsed();
    ensure(elapsed < DEADLINE, || {
        format!("cancelled sleep ran {elapsed:?}")
    })?;
    let code = sqlstate(slept, "cancelled sleep")?;
    let expected = codes(dialect).cancelled;
    ensure(code == expected, || {
        format!("cancel SQLSTATE {code}, expected {expected}")
    })?;
    ping(sleeper).await
}

/// An engine without statement cancel for other sessions says so.
async fn cannot_cancel(lister: &mut dyn Connection, id: ProcessId) -> Result<(), Failure> {
    let refused = admin(lister)?
        .signal_process(id, ProcessSignal::CancelQuery)
        .await;
    ensure(matches!(refused, Err(DriverError::Unsupported(_))), || {
        format!("cancelling another session's statement: {refused:?}")
    })
}

/// Poll the list until `id` runs a sleep, then cancel its statement.
async fn cancel_when_listed(lister: &mut dyn Connection, id: ProcessId) -> Result<(), Failure> {
    let facet = admin(lister)?;
    let deadline = Instant::now() + DEADLINE;
    let mut ticks = tokio::time::interval(POLL);
    while Instant::now() < deadline {
        ticks.tick().await;
        let processes = facet.list_processes().await?;
        let sleeping = processes.iter().any(|process| {
            process.id == id
                && !process.current
                && process
                    .query
                    .as_deref()
                    .is_some_and(|query| query.to_lowercase().contains("sleep("))
        });
        if sleeping {
            facet.signal_process(id, ProcessSignal::CancelQuery).await?;
            return Ok(());
        }
    }
    Err(Failure::Contract(format!(
        "process {id} was never listed running its sleep"
    )))
}

/// Terminating the sleeper ends its process and its connection; signalling
/// it again is the unknown-process error.
async fn terminate(
    lister: &mut dyn Connection,
    sleeper: &mut dyn Connection,
    id: ProcessId,
) -> Result<(), Failure> {
    let facet = admin(lister)?;
    facet.signal_process(id, ProcessSignal::Terminate).await?;
    let deadline = Instant::now() + DEADLINE;
    let mut ticks = tokio::time::interval(POLL);
    loop {
        ticks.tick().await;
        let processes = facet.list_processes().await?;
        if processes.iter().all(|process| process.id != id) {
            break;
        }
        ensure(Instant::now() < deadline, || {
            format!("terminated process {id} is still listed")
        })?;
    }
    let again = facet.signal_process(id, ProcessSignal::CancelQuery).await;
    let code = sqlstate(again, "signalling a terminated process")?;
    ensure(code == "42704", || {
        format!("signalling a gone process: SQLSTATE {code}, expected 42704")
    })?;
    let after = sleeper.execute("SELECT 1", &mut Collect::default()).await;
    ensure(after.is_err(), || {
        "the terminated connection still runs statements".to_owned()
    })
}
