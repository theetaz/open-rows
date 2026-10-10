// SPDX-License-Identifier: Apache-2.0
//! Separate byte/count admission reserves cancellation space under saturation.
use crate::{CONTROL_QUEUE_BYTES, Frame, PRIORITY_QUEUE_BYTES, ProtocolError, QUEUE_CAPACITY};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

/// Sender for bounded normal and reserved priority traffic.
#[derive(Clone)]
pub struct ControlQueue {
    normal: Sender,
    priority: Sender,
}
#[derive(Clone)]
struct Sender {
    queue: mpsc::Sender<QueuedFrame>,
    bytes: Arc<Semaphore>,
    slots: Arc<Semaphore>,
}
/// Admitted frame; reservations remain held until the writer finishes.
pub struct QueuedFrame {
    /// Validated frame.
    pub frame: Frame,
    _bytes: OwnedSemaphorePermit,
    _slot: OwnedSemaphorePermit,
}
/// Single control writer reads priority traffic first.
pub struct QueueReceiver {
    normal: mpsc::Receiver<QueuedFrame>,
    priority: mpsc::Receiver<QueuedFrame>,
}
fn lane(bytes: usize) -> (Sender, mpsc::Receiver<QueuedFrame>) {
    let (queue, receiver) = mpsc::channel(QUEUE_CAPACITY);
    (
        Sender {
            queue,
            bytes: Arc::new(Semaphore::new(bytes)),
            slots: Arc::new(Semaphore::new(QUEUE_CAPACITY)),
        },
        receiver,
    )
}
impl ControlQueue {
    /// Create independent queues with byte and message reservations.
    #[must_use]
    pub fn new() -> (Self, QueueReceiver) {
        let (normal, normal_receiver) = lane(CONTROL_QUEUE_BYTES);
        let (priority, priority_receiver) = lane(PRIORITY_QUEUE_BYTES);
        (
            Self { normal, priority },
            QueueReceiver {
                normal: normal_receiver,
                priority: priority_receiver,
            },
        )
    }
    /// Admit without waiting behind a full lane; saturation poisons the host.
    /// # Errors
    /// Count/byte saturation or ended writer.
    pub fn send(&self, frame: Frame) -> Result<(), ProtocolError> {
        let sender = if (20..=31).contains(&frame.header.kind.0) {
            &self.priority
        } else {
            &self.normal
        };
        let count = u32::try_from(frame.body.len() + 36).map_err(|_| ProtocolError::Limit)?;
        let bytes = sender
            .bytes
            .clone()
            .try_acquire_many_owned(count)
            .map_err(|_| ProtocolError::Limit)?;
        let slot = sender
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| ProtocolError::Limit)?;
        sender
            .queue
            .try_send(QueuedFrame {
                frame,
                _bytes: bytes,
                _slot: slot,
            })
            .map_err(|_| ProtocolError::Limit)
    }
}
impl QueueReceiver {
    /// Receive with strict preference for independent priority traffic.
    pub async fn recv(&mut self) -> Option<QueuedFrame> {
        tokio::select! { biased;
            frame = self.priority.recv() => frame,
            frame = self.normal.recv() => frame,
        }
    }
}
