// SPDX-License-Identifier: Apache-2.0
//! Small physical types still satisfy the decoder's conservative row allowance.
use arrow_array::{ArrayRef, BooleanArray, Int32Array, RecordBatch};
use openrows_host_protocol::{ArrowDecoder, ArrowEncoder, FlowKey, Grant};
use std::sync::Arc;
#[test]
fn narrow_values_use_the_same_bounded_validation_allowance()
-> Result<(), Box<dyn std::error::Error>> {
    for array in [
        Arc::new(Int32Array::from_iter_values(0..10000)) as ArrayRef,
        Arc::new(
            (0..10000)
                .map(|n| Some(n % 2 == 0))
                .collect::<BooleanArray>(),
        ),
    ] {
        let batch = RecordBatch::try_from_iter([("value", array)])?;
        let mut writer = ArrowEncoder::new(batch.schema())?;
        let encoded = writer.batch(&batch)?;
        let grant = Grant {
            key: FlowKey {
                request: 1,
                result: 1,
                batch: 1,
            },
            encoded: u32::try_from(encoded.bytes.len())?,
            decoded: u32::try_from(encoded.decoded)?,
            exclusive: false,
        };
        assert!(grant.decoded >= 80000 && grant.decoded <= 4 * 1024 * 1024);
        assert_eq!(
            ArrowDecoder::new().admit(&encoded.bytes, grant)?,
            Some(batch)
        );
    }
    Ok(())
}
