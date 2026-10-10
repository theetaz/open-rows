// SPDX-License-Identifier: Apache-2.0
//! Blocking worker fragments meet an independent async data writer.
use openrows_host_client::transport::Stream;
use openrows_host_protocol::{
    Control, ControlQueue, FRAGMENT_LIMIT, Frame, Header, Lane, MessageKind, ProtocolError,
    write_frame,
};
use std::sync::{Arc, mpsc};
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) struct Output {
    pub(crate) generation: u64,
    pub(crate) request: u64,
    pub(crate) control: ControlQueue,
    pub(crate) data: mpsc::SyncSender<DataWrite>,
}
impl Output {
    pub(crate) fn control(&self, result: u32, control: &Control) -> Result<(), ProtocolError> {
        let mut frame = Frame::control(self.generation, self.request, control)?;
        frame.header.result = result;
        self.control.send(frame)
    }
    pub(crate) fn data(
        &self,
        result: u32,
        sequence: &mut u64,
        bytes: Vec<u8>,
    ) -> Result<(), ProtocolError> {
        let count = u64::try_from(bytes.len().div_ceil(FRAGMENT_LIMIT))
            .map_err(|_| ProtocolError::Limit)?;
        let last = sequence.checked_add(count).ok_or(ProtocolError::Limit)?;
        let header = Header {
            kind: MessageKind(100),
            generation: self.generation,
            request: self.request,
            result,
            sequence: *sequence,
        };
        // Move the one authorized segment once; fragmenting in the writer avoids a wakeup per 64 KiB.
        self.data
            .send(DataWrite {
                header,
                bytes: Zeroizing::new(bytes),
            })
            .map_err(|_| ProtocolError::Transport)?;
        *sequence = last;
        Ok(())
    }
}
pub(crate) struct DataWrite {
    header: Header,
    bytes: Zeroizing<Vec<u8>>,
}

pub(crate) fn writer(
    mut data: Stream,
    receiver: &mpsc::Receiver<DataWrite>,
) -> Result<(), ProtocolError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ProtocolError::Transport)?;
    while let Ok(segment) = receiver.recv() {
        let mut header = segment.header;
        for chunk in segment.bytes.chunks(FRAGMENT_LIMIT) {
            header.sequence += 1;
            let frame = Frame {
                header,
                body: Zeroizing::new(chunk.to_vec()),
            };
            runtime.block_on(write_frame(&mut data, &frame, Lane::Data))?;
        }
    }
    Ok(())
}

pub(crate) fn start(data: Stream) -> Result<mpsc::SyncSender<DataWrite>, ProtocolError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("host-data".into())
        .spawn(move || {
            if writer(data, &receiver).is_err() {
                std::process::exit(70);
            }
        })
        .map_err(|_| ProtocolError::Transport)?;
    Ok(sender)
}
pub(crate) fn control_writer(
    mut write: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
    mut outgoing: openrows_host_protocol::QueueReceiver,
    done: tokio::sync::mpsc::Sender<u64>,
    acknowledged: Arc<tokio::sync::Notify>,
) -> tokio::task::JoinHandle<Result<(), ProtocolError>> {
    tokio::spawn(async move {
        while let Some(frame) = outgoing.recv().await {
            write_frame(&mut write, &frame.frame, Lane::Control).await?;
            if matches!(frame.frame.header.kind.0, 41 | 42 | 48) {
                done.send(frame.frame.header.request)
                    .await
                    .map_err(|_| ProtocolError::Transport)?;
            }
            if frame.frame.header.kind == Control::CloseAck.kind() {
                acknowledged.notify_one();
            }
        }
        Ok(())
    })
}
