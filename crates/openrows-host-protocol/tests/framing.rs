#![allow(clippy::unwrap_used, reason = "synthetic fixtures and test assertions")]
// SPDX-License-Identifier: Apache-2.0
//! Hostile lengths, fragments and arbitrary bytes never request unbounded memory.
use openrows_host_protocol::*;
use proptest::prelude::*;
#[test]
fn fragmented_frames_reassemble_and_truncation_is_typed() -> Result<(), ProtocolError> {
    let frame = Frame::control(
        1,
        1,
        &Control::Execute {
            sql: "test-only".into(),
        },
    )?
    .encode(Lane::Control)?;
    for split in 1..frame.len() {
        let mut decoder = FrameDecoder::new(Lane::Control);
        assert!(decoder.push(&frame[..split])?.is_empty());
        assert_eq!(decoder.finish(), Err(ProtocolError::Truncated));
        let result = decoder.push(&frame[split..])?;
        assert_eq!(result.len(), 1);
        assert_eq!(decoder.finish(), Ok(()));
    }
    Ok(())
}
#[test]
fn oversized_unknown_and_malformed_frames_are_typed() -> Result<(), ProtocolError> {
    let oversized = u32::MAX.to_le_bytes();
    assert!(matches!(
        FrameDecoder::new(Lane::Control).push(&oversized),
        Err(ProtocolError::Limit)
    ));
    let mut bytes = Frame::control(1, 1, &Control::Ping)?.encode(Lane::Control)?;
    bytes[6..8].copy_from_slice(&65535_u16.to_le_bytes());
    assert!(matches!(
        FrameDecoder::new(Lane::Control).push(&bytes),
        Err(ProtocolError::UnknownKind)
    ));
    bytes[6..8].copy_from_slice(&13_u16.to_le_bytes());
    bytes[36] = 255;
    let frames = FrameDecoder::new(Lane::Control).push(&bytes)?;
    assert!(matches!(
        frames[0].decode_control(),
        Err(ProtocolError::Invalid)
    ));
    Ok(())
}
#[test]
fn stale_generations_ids_sequences_and_duplicate_terminals_fail() {
    let mut tracker = RequestTracker::new(2);
    assert_eq!(tracker.begin(1, 1), Err(ProtocolError::Stale));
    tracker.begin(2, 1).unwrap();
    assert_eq!(tracker.begin(2, 1), Err(ProtocolError::Stale));
    let mut header = Header {
        kind: MessageKind(100),
        generation: 2,
        request: 1,
        result: 1,
        sequence: 2,
    };
    assert_eq!(tracker.data(header), Err(ProtocolError::Stale));
    header.sequence = 1;
    tracker.data(header).unwrap();
    header.generation = 1;
    assert_eq!(tracker.data(header), Err(ProtocolError::Stale));
    assert_eq!(tracker.finish(1, (1, 2)), Err(ProtocolError::Stale));
    tracker.finish(1, (1, 1)).unwrap();
    assert_eq!(tracker.finish(1, (1, 1)), Err(ProtocolError::Stale));
}
proptest! {
    #[test]
    fn arbitrary_decoder_input_is_bounded(bytes in prop::collection::vec(any::<u8>(), 0..8192)) {
        let mut decoder = FrameDecoder::new(Lane::Control);
        for piece in bytes.chunks(17) { if decoder.push(piece).is_err() { break; } }
        let _ = decoder.finish();
    }
}
#[tokio::test]
async fn priority_has_reserved_bytes_and_count_even_with_normal_queue_full()
-> Result<(), ProtocolError> {
    let (queue, mut receiver) = ControlQueue::new();
    for request in 1..=QUEUE_CAPACITY {
        queue.send(Frame::control(1, request as u64, &Control::Ping)?)?;
    }
    assert!(matches!(
        queue.send(Frame::control(1, 33, &Control::Ping)?),
        Err(ProtocolError::Limit)
    ));
    queue.send(Frame::control(1, 0, &Control::Cancel { target: 1 })?)?;
    assert_eq!(
        receiver.recv().await.unwrap().frame.decode_control()?,
        Control::Cancel { target: 1 }
    );
    let oversized = Control::Execute {
        sql: "x".repeat(CONTROL_LIMIT),
    };
    assert!(matches!(
        Frame::control(1, 34, &oversized),
        Err(ProtocolError::Limit)
    ));
    Ok(())
}
#[test]
fn one_batch_credit_pause_resume_and_late_consumption() {
    let key = FlowKey {
        request: 1,
        result: 1,
        batch: 1,
    };
    let grant = Grant {
        key,
        encoded: 100,
        decoded: 100,
        exclusive: false,
    };
    let mut state = CreditState::default();
    state.offer(grant).unwrap();
    state.pause();
    state.grant(grant, false).unwrap();
    assert!(!state.authorized());
    state.grant(grant, true).unwrap();
    assert!(state.authorized());
    state.pause();
    assert!(state.authorized());
    state.sent().unwrap();
    state.grant(grant, true).unwrap();
    assert!(!state.authorized());
    state.consume(key).unwrap();
    state.consume(key).unwrap();
    state.grant(grant, true).unwrap();
    let invalid = Grant {
        encoded: u32::try_from(LARGE_LIMIT).unwrap() + 1,
        exclusive: true,
        ..grant
    };
    assert_eq!(state.offer(invalid), Err(ProtocolError::Limit));
}
