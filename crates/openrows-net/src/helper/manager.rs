// SPDX-License-Identifier: Apache-2.0
//! Weak profile pool: leases own helpers; one restart per leased lifetime.
use super::{
    HelperError, HelperSpec,
    process::{Process, kill_tree, local_endpoint},
    stderr::Stderr,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc, Weak,
        atomic::{AtomicU32, Ordering},
    },
};
use tokio::sync::{Mutex, watch};

#[derive(Clone, Debug)]
pub(super) enum Status {
    Ready,
    Restarting,
    Failed(HelperError),
}

/// Shared running helper, released after its last dialer or stream closes.
pub struct Helper {
    pub(super) address: SocketAddr,
    pub(super) spec: HelperSpec,
    pub(super) status: watch::Receiver<Status>,
    stderr: Stderr,
    stop: watch::Sender<bool>,
    finished: watch::Receiver<bool>,
    pid: Arc<AtomicU32>,
}
impl Helper {
    /// PID of the current child (changes on its single restart).
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid.load(Ordering::Acquire)
    }
    /// Local TCP endpoint; TLS still targets the original database hostname.
    #[must_use]
    pub fn endpoint(&self) -> SocketAddr {
        self.address
    }
    /// Last bounded, scrubbed stderr lines.
    #[must_use]
    pub fn stderr(&self) -> String {
        self.stderr.text()
    }
    /// Exact argument display and detected version for Test connection.
    ///
    /// # Errors
    /// Invalid hop arguments.
    pub fn diagnostics(&self) -> Result<String, HelperError> {
        Ok(format!(
            "{}\nVersion: {}",
            self.spec.command_line(self.address.port())?,
            self.spec.version
        ))
    }
    pub(super) async fn ready(&self) -> Result<(), HelperError> {
        let mut status = self.status.clone();
        loop {
            let current = status.borrow_and_update().clone();
            match current {
                Status::Ready => return Ok(()),
                Status::Failed(error) => return Err(error),
                Status::Restarting => {}
            }
            status
                .changed()
                .await
                .map_err(|_| self.spec.failure("Helper stopped", &self.stderr))?;
        }
    }
    pub(super) fn failure(&self) -> HelperError {
        match &*self.status.borrow() {
            Status::Failed(error) => error.clone(),
            Status::Ready | Status::Restarting => self.spec.failure(
                "Helper connection closed; reconnect the database session",
                &self.stderr,
            ),
        }
    }
    /// Kill the tree and wait for the child to be reaped.
    pub async fn close(&self) {
        self.stop.send_replace(true);
        kill_tree(self.pid());
        let mut finished = self.finished.clone();
        while !*finished.borrow_and_update() {
            if finished.changed().await.is_err() {
                break;
            }
        }
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        kill_tree(self.pid());
    }
}

/// App-owned helper pool; it retains no leases after sessions close.
pub struct Helpers {
    pool: Mutex<Pool>,
    shutdown: watch::Sender<bool>,
}
impl Default for Helpers {
    fn default() -> Self {
        Self {
            pool: Mutex::default(),
            shutdown: watch::channel(false).0,
        }
    }
}
#[derive(Default)]
struct Pool {
    entries: HashMap<String, Weak<Helper>>,
    live: Vec<Weak<Helper>>,
    shutdown: bool,
}
impl Helpers {
    /// Acquire one helper for this saved profile; concurrent callers share startup.
    ///
    /// # Errors
    /// Tool launch, readiness or shutdown failure.
    pub async fn acquire(
        &self,
        profile: &str,
        spec: HelperSpec,
    ) -> Result<Arc<Helper>, HelperError> {
        let mut cancelled = self.shutdown.subscribe();
        let mut pool = self.pool.lock().await;
        if pool.shutdown {
            return Err(spec.failure("App is shutting down", &Stderr::default()));
        }
        pool.entries.retain(|_, helper| helper.strong_count() > 0);
        if let Some(helper) = pool.entries.get(profile).and_then(Weak::upgrade)
            && helper.spec.hop == spec.hop
            && helper.spec.path == spec.path
        {
            tokio::select! {
                result = helper.ready() => result?,
                _ = cancelled.changed() => return Err(spec.failure("App is shutting down", &Stderr::default())),
            }
            return Ok(helper);
        }
        if *cancelled.borrow() {
            return Err(spec.failure("App is shutting down", &Stderr::default()));
        }
        let helper = tokio::select! {
            result = start(spec.clone()) => result?,
            _ = cancelled.changed() => return Err(spec.failure("App is shutting down", &Stderr::default())),
        };
        pool.live.retain(|helper| helper.strong_count() > 0);
        pool.live.push(Arc::downgrade(&helper));
        pool.entries.insert(profile.into(), Arc::downgrade(&helper));
        Ok(helper)
    }
    /// App exit hook: kill every live tree and reap before returning.
    pub async fn shutdown(&self) {
        self.shutdown.send_replace(true);
        let helpers = {
            let mut pool = self.pool.lock().await;
            pool.shutdown = true;
            pool.entries.clear();
            pool.live
                .drain(..)
                .filter_map(|helper| helper.upgrade())
                .collect::<Vec<_>>()
        };
        for helper in helpers {
            helper.close().await;
        }
    }
}

async fn start(spec: HelperSpec) -> Result<Arc<Helper>, HelperError> {
    let address = local_endpoint()?;
    let stderr = Stderr::default();
    let mut process = Process::spawn(&spec, address.port(), &stderr)?;
    if let Err(error) = process.ready(&spec, address, &stderr).await {
        process.stop().await;
        return Err(match error {
            HelperError::Network { message, .. } => spec.failure(
                format!(
                    "{message}\n{}\nVersion: {}",
                    spec.command_line(address.port())?,
                    spec.version
                ),
                &stderr,
            ),
            other => other,
        });
    }
    let (stop, stopping) = watch::channel(false);
    let (status, receiver) = watch::channel(Status::Ready);
    let (finished, completion) = watch::channel(false);
    let pid = Arc::new(AtomicU32::new(process.pid));
    let helper = Arc::new(Helper {
        address,
        spec,
        status: receiver,
        stderr,
        stop,
        finished: completion,
        pid,
    });
    tokio::spawn(supervise(Supervisor {
        process,
        helper: Arc::downgrade(&helper),
        stop: stopping,
        status,
        finished,
    }));
    Ok(helper)
}

struct Supervisor {
    process: Process,
    helper: Weak<Helper>,
    stop: watch::Receiver<bool>,
    status: watch::Sender<Status>,
    finished: watch::Sender<bool>,
}
async fn supervise(supervisor: Supervisor) {
    let Supervisor {
        mut process,
        helper,
        mut stop,
        status,
        finished,
    } = supervisor;
    let mut restarted = false;
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            result = process.child.wait() => {
                kill_tree(process.pid);
                process.drain().await;
                let Some(lease) = helper.upgrade() else { break; };
                if *stop.borrow() { break; }
                if restarted {
                    status.send_replace(Status::Failed(lease.spec.failure(format!("Exited again after one restart: {result:?}"), &lease.stderr)));
                    break;
                }
                restarted = true;
                status.send_replace(Status::Restarting);
                let spec = lease.spec.clone();
                let address = lease.address;
                let stderr = lease.stderr.clone();
                let pid = Arc::clone(&lease.pid);
                drop(lease);
                match restart((spec, address, stderr), &mut stop).await {
                    Ok(fresh) => { pid.store(fresh.pid, Ordering::Release); process = fresh; status.send_replace(Status::Ready); }
                    Err(error) => { status.send_replace(Status::Failed(error)); break; }
                }
            }
        }
    }
    process.stop().await;
    if let Some(lease) = helper.upgrade()
        && !matches!(*status.borrow(), Status::Failed(_))
    {
        status.send_replace(Status::Failed(
            lease.spec.failure("Helper stopped", &lease.stderr),
        ));
    }
    finished.send_replace(true);
}

async fn restart(
    context: (HelperSpec, SocketAddr, Stderr),
    stop: &mut watch::Receiver<bool>,
) -> Result<Process, HelperError> {
    let (spec, address, stderr) = context;
    let mut process = Process::spawn(&spec, address.port(), &stderr)?;
    let ready = tokio::select! {
        result = process.ready(&spec, address, &stderr) => result,
        _ = stop.changed() => Err(spec.failure("Restart cancelled", &stderr)),
    };
    if let Err(error) = ready {
        process.stop().await;
        return Err(error);
    }
    Ok(process)
}
