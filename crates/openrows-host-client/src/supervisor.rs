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
}
impl HostSupervisor {
    /// Preserve explicit helper path and shared budget/profile circuit.
    #[must_use]
    pub fn new(options: LaunchOptions) -> Self {
        Self { options }
    }
    /// Start exactly one hosted connection lifetime, without retry or fallback.
    /// # Errors
    /// Spawn, memory, handshake, compatibility or circuit failure.
    pub async fn spawn(&self) -> Result<HostSession, HostFailure> {
        self.options.circuit.check()?;
        let reservation = self.options.memory.host()?;
        let mut rendezvous = Rendezvous::new(&self.options.endpoint_root)?;
        let mut child = launch_command(&self.options)?
            .spawn()
            .map_err(|_| HostFailure::Spawn)?;
        let mut tasks = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            tasks.push(tokio::spawn(discard(stdout)));
        }
        if let Some(stderr) = child.stderr.take() {
            tasks.push(tokio::spawn(discard(stderr)));
        }
        let stdin = child.stdin.take();
        let handshake = async {
            let mut stdin = stdin.ok_or(HostFailure::Spawn)?;
            crate::write_bootstrap(&mut stdin, &rendezvous.bootstrap)
                .await
                .map_err(HostFailure::from)?;
            drop(stdin);
            rendezvous.attach(self.options.driver).await
        };
        let attached = tokio::select! {
            result = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake) =>
                result.unwrap_or(Err(HostFailure::Unresponsive)),
            _ = child.wait() => Err(HostFailure::Incompatible),
        };
        match attached {
            Ok((control, data)) => {
                // Named pipes do not use this directory. Remove it before kill-on-parent-close is armed.
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
                let (mut session, channels) = channels(control, data, generation, &self.options);
                tasks.extend(channels.tasks);
                tokio::spawn(
                    Monitor {
                        child,
                        failure: session.failure.clone(),
                        directory: rendezvous.directory,
                        reservation,
                        large: session.large.clone(),
                        platform: guard,
                        circuit: self.options.circuit.clone(),
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
                Ok(session)
            }
            Err(error) => {
                cleanup(&mut child, tasks).await;
                self.options.circuit.record_exit();
                Err(error)
            }
        }
    }
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
