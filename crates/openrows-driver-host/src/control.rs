// SPDX-License-Identifier: Apache-2.0
//! Control runtime never waits for data credit or locks a busy connection.
use crate::{
    flow::Flow,
    output::Output,
    worker::{Active, Worker},
};
use openrows_host_client::transport::Stream;
use openrows_host_protocol::{
    Control, ControlQueue, Frame, Lane, ProtocolError, RequestTracker, read_frame,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, oneshot};

struct Runtime {
    generation: u64,
    control: ControlQueue,
    flow: Arc<Flow>,
    active: Arc<Mutex<Active>>,
    authorized: Arc<AtomicU64>,
    jobs: ControlQueue,
    tracker: RequestTracker,
}
pub(crate) async fn run(
    control: Stream,
    data: Stream,
    generation: u64,
) -> Result<(), ProtocolError> {
    let (queue, outgoing) = ControlQueue::new();
    let (done, mut completed) = tokio::sync::mpsc::channel(32);
    let (read, write) = tokio::io::split(control);
    let (frames, reader) = input(read);
    let written = Arc::new(Notify::new());
    let writer = crate::output::control_writer(write, outgoing, done, written.clone());
    let (mut runtime, mut stopped) = Runtime::start(data, generation, queue)?;
    runtime
        .serve(&mut Events {
            frames,
            completed: &mut completed,
            stopped: &mut stopped,
        })
        .await?;
    reader.abort();
    runtime.flow.close();
    drop(runtime.jobs);
    written.notified().await;
    let outcome = tokio::time::timeout(openrows_host_client::CLOSE_GRACE, stopped)
        .await
        .map_err(|_| ProtocolError::Transport)?
        .map_err(|_| ProtocolError::Transport)?;
    writer.abort();
    outcome
}
impl Runtime {
    fn start(
        data: Stream,
        generation: u64,
        control: ControlQueue,
    ) -> Result<(Self, oneshot::Receiver<Result<(), ProtocolError>>), ProtocolError> {
        let flow = Arc::new(Flow::default());
        let active = Arc::new(Mutex::new(Active::new()));
        let authorized = Arc::new(AtomicU64::new(0));
        let (jobs, commands) = ControlQueue::new();
        let output = Output {
            generation,
            request: 0,
            control: control.clone(),
            data: crate::output::start(data)?,
        };
        let worker = Worker {
            commands,
            output,
            flow: flow.clone(),
            active: active.clone(),
            authorized: authorized.clone(),
        };
        let stopped = crate::worker::start(worker)?;
        let runtime = Self {
            generation,
            control,
            flow,
            active,
            authorized,
            jobs,
            tracker: RequestTracker::new(generation),
        };
        Ok((runtime, stopped))
    }
    async fn serve(&mut self, events: &mut Events<'_>) -> Result<(), ProtocolError> {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                frame = events.frames.recv() => { if self.receive(frame.ok_or(ProtocolError::Transport)??).await? { return Ok(()); } }
                _ = heartbeat.tick() => {
                    #[cfg(feature = "test-hooks")]
                    if !crate::hooks::heartbeating() { continue; }
                    self.reply(0, &Control::Heartbeat)?;
                }
                request = events.completed.recv() => {
                    self.tracker.finish(request.ok_or(ProtocolError::Transport)?, (0, 0))?;
                }
                _ = &mut *events.stopped => return Err(ProtocolError::Transport),
            }
        }
    }
    fn reply(&self, request: u64, control: &Control) -> Result<(), ProtocolError> {
        self.control
            .send(Frame::control(self.generation, request, control)?)
    }
    async fn receive(&mut self, frame: Frame) -> Result<bool, ProtocolError> {
        if frame.header.generation != self.generation {
            return Err(ProtocolError::Stale);
        }
        let control = frame.decode_control()?;
        match &control {
            Control::Cancel { target } => {
                let cancellation = self
                    .active
                    .lock()
                    .map_err(|_| ProtocolError::Invalid)?
                    .cancel(*target);
                let requested = cancellation.is_some();
                if let Some(cancellation) = cancellation {
                    self.flow.cancel(*target);
                    cancellation.await.map_err(|_| ProtocolError::Invalid)?;
                }
                self.reply(
                    frame.header.request,
                    &Control::CancelAck {
                        target: *target,
                        requested,
                    },
                )?;
            }
            Control::Close => {
                self.flow.close();
                self.reply(frame.header.request, &Control::CloseAck)?;
                return Ok(true);
            }
            Control::Heartbeat => {
                #[cfg(feature = "test-hooks")]
                if !crate::hooks::heartbeating() {
                    return Ok(false);
                }
                self.reply(frame.header.request, &Control::Heartbeat)?;
            }
            Control::Grant(_)
            | Control::Resume(_)
            | Control::Consumed(_)
            | Control::Stop(_)
            | Control::Pause(_) => {
                self.flow.apply(&control)?;
                if let Control::Pause(key) = control {
                    self.reply(
                        frame.header.request,
                        &Control::Paused {
                            key,
                            sequence: self.authorized.load(Ordering::SeqCst),
                        },
                    )?;
                }
            }
            Control::Connect(_)
            | Control::Execute { .. }
            | Control::ExecuteBound { .. }
            | Control::Ping
            | Control::Catalog { .. }
            | Control::Editor
            | Control::Admin
            | Control::Bulk => {
                self.tracker.begin(self.generation, frame.header.request)?;
                self.active
                    .lock()
                    .map_err(|_| ProtocolError::Invalid)?
                    .pending
                    .insert(frame.header.request, false);
                self.reply(frame.header.request, &Control::Accepted)?;
                self.jobs.send(frame).map_err(|_| ProtocolError::Limit)?;
            }
            _ => return Err(ProtocolError::Invalid),
        }
        Ok(false)
    }
}

struct Events<'a> {
    frames: tokio::sync::mpsc::Receiver<Result<Frame, ProtocolError>>,
    completed: &'a mut tokio::sync::mpsc::Receiver<u64>,
    stopped: &'a mut oneshot::Receiver<Result<(), ProtocolError>>,
}
// A separate reader preserves partial frame bytes when heartbeat/completion wins select.
fn input(
    mut read: impl tokio::io::AsyncRead + Unpin + Send + 'static,
) -> (
    tokio::sync::mpsc::Receiver<Result<Frame, ProtocolError>>,
    tokio::task::JoinHandle<()>,
) {
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let task = tokio::spawn(async move {
        loop {
            let frame = read_frame(&mut read, Lane::Control).await;
            let failed = frame.is_err();
            if sender.send(frame).await.is_err() || failed {
                break;
            }
        }
    });
    (receiver, task)
}
