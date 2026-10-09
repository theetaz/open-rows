// SPDX-License-Identifier: Apache-2.0
use std::borrow::Cow;

use super::*;
use crate::tds::types::{Layout, SqlType, TypeInfo};

fn column(sql: SqlType) -> Column {
    Column {
        name: "c".to_owned(),
        info: TypeInfo {
            sql,
            layout: Layout::ByteLen,
        },
        user_type: 0,
    }
}

/// `datetime2(7)` wire bytes: ticks of 100 ns, then days since 0001-01-01.
fn stamp(ticks: u64, days: u32) -> Vec<u8> {
    let mut bytes = ticks.to_le_bytes()[..5].to_vec();
    bytes.extend(&days.to_le_bytes()[..3]);
    bytes
}

#[test]
fn values_append_and_nulls_stay_null() -> Result<(), DriverError> {
    let mut batch = BatchBuilder::new(&[
        column(SqlType::Int),
        column(SqlType::NChar { varying: true }),
    ]);
    batch.append(&vec![
        Some(Cow::Owned(5_i32.to_le_bytes().to_vec())),
        Some(Cow::Owned(vec![b'h', 0])),
    ])?;
    batch.append(&vec![None, None])?;
    let finished = batch.finish()?;
    assert_eq!(finished.num_rows(), 2);
    assert_eq!(finished.column(1).as_string::<i32>().value(0), "h");
    assert!(finished.column(0).is_null(1));
    assert!(batch.append(&vec![Some(Cow::Owned(vec![1]))]).is_err());
    Ok(())
}

#[test]
fn an_out_of_range_datetime2_turns_its_column_into_tagged_text() -> Result<(), DriverError> {
    let mut batch = BatchBuilder::new(&[column(SqlType::DateTime2 { scale: 7 })]);
    batch.append(&vec![Some(Cow::Owned(stamp(1, 719_162)))])?;
    batch.append(&vec![None])?;
    batch.append(&vec![Some(Cow::Owned(stamp(0, 3_652_058)))])?;
    let finished = batch.finish()?;
    assert_eq!(finished.schema().field(0).data_type(), &DataType::Utf8);
    let texts = finished.column(0).as_string::<i32>();
    assert_eq!(texts.value(0), "1970-01-01 00:00:00.0000001");
    assert!(texts.is_null(1));
    assert_eq!(texts.value(2), "9999-12-31 00:00:00.0000000");
    batch.append(&vec![Some(Cow::Owned(stamp(0, 719_162)))])?;
    assert_eq!(
        batch.finish()?.schema().field(0).data_type(),
        &DataType::Utf8
    );
    Ok(())
}
