// SPDX-License-Identifier: Apache-2.0
//! Spawn ownership moves to a monitor only after both lanes authenticate.
use crate::{
    HANDSHAKE_TIMEOUT, HostFailure, HostSession, HostState, LaunchOptions, handshake::Rendezvous,
    launch_command, monitor::Monitor, platform, session::Shutdown,
};
use openrows_host_protocol::{Control, ControlQueue, Lane, read_frame, write_frame};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    sync::{mpsc, watch},
    time::Instant,
};

/// Reusable lazy launcher; construction never spawns or opens credentials.
#[derive(Clone)]
pub struct HostSupervisor {
    options: LaunchOptions,
    prewarm: Option<crate::prewarm::Prewarm>,
}
impl HostSupervisor {
    /// Preserve explicit helper path and shared budget/profile circuit.
    #[must_use]
    pub fn new(options: LaunchOptions) -> Self {
        Self {
            options,
            prewarm: None,
        }
    }
    pub(crate) fn with_prewarm(mut self, prewarm: crate::prewarm::Prewarm) -> Self {
        self.prewarm = Some(prewarm);
        self
    }
    /// Start exactly one hosted connection lifetime, without retry or fallback.
    /// # Errors
    /// Spawn, memory, handshake, compatibility or circuit failure.
    pub async fn spawn(&self) -> Result<HostSession, HostFailure> {
        if let Some(prewarm) = &self.prewarm {
            prewarm.wait().await;
        }
        self.options.circuit.check()?;
        let reservation = self.options.memory.host()?;
        let mut rendezvous = Rendezvous::new(&self.options.endpoint_root)?;
        let started = Instant::now();
        let mut child = launch_command(&self.options)?
            .spawn()
            .map_err(|_| HostFailure::Spawn)?;
        let mut tasks = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            tasks.push(tokio::spawn(discard(stdout)));
        }
        let diagnostic = child.stderr.take().map(crate::startup::Diagnostics::start);
        let stdin = child.stdin.take();
        let handshake = async {
            let mut stdin = stdin.ok_or(HostFailure::Spawn)?;
            crate::handshake::bootstrap_host(&mut stdin, &rendezvous.bootstrap).await?;
            drop(stdin);
            rendezvous.attach(self.options.driver).await
        };
        let mut exit = None;
        let attached = tokio::select! {
            result = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake) =>
                result.unwrap_or(Err(HostFailure::Unresponsive)),
            status = child.wait() => {
                exit = Some(status);
                Err(HostFailure::Incompatible)
            },
        };
        match attached {
            Ok(lanes) => {
                if let Some(task) = diagnostic.and_then(crate::startup::Diagnostics::into_task) {
                    tasks.push(task);
                }
                supervise(
                    &self.options,
                    Pending {
                        child,
                        rendezvous,
                        reservation,
                        tasks,
                    },
                    lanes,
                    started.elapsed(),
                )
                .await
            }
            Err(error) => {
                cleanup(&mut child, tasks).await;
                crate::startup::failure(error, self.options.driver, exit, diagnostic).await;
                self.options.circuit.record_exit();
                Err(error)
            }
        }
    }
}
struct Pending {
    child: tokio::process::Child,
    rendezvous: Rendezvous,
    reservation: crate::HostReservation,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
async fn supervise(
    options: &LaunchOptions,
    pending: Pending,
    lanes: (crate::transport::Stream, crate::transport::Stream),
    handshake: std::time::Duration,
) -> Result<HostSession, HostFailure> {
    let Pending {
        mut child,
        rendezvous,
        reservation,
        mut tasks,
    } = pending;
    // Remove unused Windows rendezvous before arming kill-on-parent-close.
    #[cfg(windows)]
    if std::fs::remove_dir(rendezvous.directory.path()).is_err() {
        cleanup(&mut child, tasks).await;
        return Err(HostFailure::Spawn);
    }
    let guard = match platform::Guard::attach(&child) {
        Ok(guard) => guard,
        Err(error) => {
            cleanup(&mut child, tasks).await;
            return Err(error);
        }
    };
    let pid = child.id().ok_or(HostFailure::Exited)?;
    let policy = guard.policy();
    let generation = rendezvous.bootstrap.generation;
    let directory = rendezvous.directory.path().to_path_buf();
    let (mut session, channels) = channels(lanes.0, lanes.1, generation, options);
    tasks.extend(channels.tasks);
    tokio::spawn(
        Monitor {
            child,
            failure: session.failure.clone(),
            directory: rendezvous.directory,
            reservation,
            large: session.large.clone(),
            platform: guard,
            circuit: options.circuit.clone(),
            generation,
            control: session.outgoing.clone(),
            shutdown: channels.shutdown,
            heartbeat: channels.heartbeat,
            state: channels.state,
            tasks,
        }
        .run(),
    );
    session.pid = pid;
    session.directory = directory;
    session.policy = policy;
    session.handshake = handshake;
    Ok(session)
}
struct Channels {
    shutdown: mpsc::Receiver<Shutdown>,
    heartbeat: watch::Receiver<Instant>,
    state: watch::Sender<HostState>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
fn channels(
    control: crate::transport::Stream,
    data: crate::transport::Stream,
    generation: u64,
    options: &LaunchOptions,
) -> (HostSession, Channels) {
    let (outgoing, writer) = ControlQueue::new();
    let (incoming, reader) = ControlQueue::new();
    let (shutdown, requests) = mpsc::channel(64);
    let failure = std::sync::Arc::new(crate::failure_signal::FailureSignal::new(shutdown.clone()));
    let (state, observe) = watch::channel(HostState::Ready);
    let (heartbeat, beats) = watch::channel(Instant::now());
    let (read, write) = tokio::io::split(control);
    let tasks = vec![
        tokio::spawn(control_write(write, writer, failure.clone())),
        tokio::spawn(control_read(
            read,
            ReaderSignals {
                incoming,
                failure: failure.clone(),
                heartbeat,
                shutdown: shutdown.clone(),
                generation,
            },
        )),
    ];
    (
        HostSession {
            handshake: std::time::Duration::ZERO,
            failure,
            generation,
            pid: 0,
            directory: std::path::PathBuf::new(),
            policy: "",
            outgoing,
            incoming: reader,
            data,
            state: observe,
            shutdown,
            heartbeat: beats.clone(),
            memory: options.memory.clone(),
            large: std::sync::Arc::default(),
        },
        Channels {
            shutdown: requests,
            heartbeat: beats,
            state,
            tasks,
        },
    )
}
async fn control_write(
    mut stream: impl AsyncWrite + Unpin,
    mut queue: openrows_host_protocol::QueueReceiver,
    failure: std::sync::Arc<crate::failure_signal::FailureSignal>,
) {
    while let Some(queued) = queue.recv().await {
        if let Err(error) = write_frame(&mut stream, &queued.frame, Lane::Control).await {
            failure.fail(error.into());
            break;
        }
    }
}
struct ReaderSignals {
    incoming: ControlQueue,
    failure: std::sync::Arc<crate::failure_signal::FailureSignal>,
    heartbeat: watch::Sender<Instant>,
    shutdown: mpsc::Sender<Shutdown>,
    generation: u64,
}
async fn control_read(mut stream: impl AsyncRead + Unpin, signals: ReaderSignals) {
    let ReaderSignals {
        incoming,
        failure,
        heartbeat,
        shutdown,
        generation,
    } = signals;
    let outcome: Result<(), HostFailure> = async {
        loop {
            let frame = read_frame(&mut stream, Lane::Control)
                .await
                .map_err(crate::session::transport_failure)?;
            if frame.header.generation != generation {
                return Err(HostFailure::Protocol);
            }
            match frame.decode_control().map_err(HostFailure::from)? {
                Control::Heartbeat => {
                    heartbeat.send_replace(Instant::now());
                }
                Control::Error(openrows_host_protocol::WireError::Protocol) => {
                    return Err(HostFailure::Protocol);
                }
                Control::Reply(_) | Control::Error(_) | Control::ConnectError { .. } => {
                    shutdown
                        .try_send(Shutdown::Completed(frame.header.request))
                        .map_err(|_| HostFailure::Protocol)?;
                    incoming.send(frame).map_err(HostFailure::from)?;
                }
                _ => incoming.send(frame).map_err(HostFailure::from)?,
            }
        }
    }
    .await;
    if let Err(reason) = outcome {
        failure.fail(reason);
    }
}
async fn cleanup(child: &mut tokio::process::Child, tasks: Vec<tokio::task::JoinHandle<()>>) {
    let _ = child.start_kill();
    let _ = child.wait().await;
    for task in tasks {
        task.abort();
        let _ = task.await;
    }
}
async fn discard(mut reader: impl AsyncRead + Unpin) {
    let mut buffer = [0; 4096];
    while reader.read(&mut buffer).await.is_ok_and(|count| count != 0) {}
}
