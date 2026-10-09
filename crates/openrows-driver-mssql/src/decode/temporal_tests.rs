// SPDX-License-Identifier: Apache-2.0
use super::*;

/// Wire bytes of `date` for days since 0001-01-01.
fn date(days: u32) -> Vec<u8> {
    days.to_le_bytes()[..3].to_vec()
}

#[test]
fn dates_count_days_from_year_one() -> Result<(), CellError> {
    assert_eq!(date_days(&date(719_162))?, 0);
    assert_eq!(date_days(&date(0))?, -719_162);
    let mut text = String::new();
    write_date(date_days(&date(0))?, &mut text);
    assert_eq!(text, "0001-01-01");
    text.clear();
    write_date(date_days(&date(3_652_058))?, &mut text);
    assert_eq!(text, "9999-12-31");
    assert!(date_days(&[1, 2]).is_err());
    Ok(())
}

#[test]
fn times_scale_to_nanoseconds() -> Result<(), CellError> {
    // 13:05:00.25 as time(2): 4_710_025 hundredths in three bytes.
    let ticks = 4_710_025_u32.to_le_bytes();
    assert_eq!(time_nanos(&ticks[..3], 2)?, 47_100_250_000_000);
    let mut text = String::new();
    write_clock(47_100_250_000_000, 2, &mut text);
    assert_eq!(text, "13:05:00.25");
    let seven = 1_234_567_u64.to_le_bytes();
    assert_eq!(time_nanos(&seven[..5], 7)?, 123_456_700);
    assert!(time_nanos(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF], 7).is_err());
    Ok(())
}

#[test]
fn datetime2_maps_to_epoch_units_and_falls_back_outside_nanos() -> Result<(), CellError> {
    let mut bytes = 5_u64.to_le_bytes()[..5].to_vec();
    bytes.extend(date(719_162));
    assert_eq!(datetime2_nanos(&bytes, 7)?, 500);
    assert_eq!(datetime2_micros(&bytes, 7)?, 0);
    let mut far = vec![0; 5];
    far.extend(date(3_652_058));
    assert_eq!(
        datetime2_nanos(&far, 7),
        Err(CellError::Unrepresentable("datetime2"))
    );
    assert!(datetime2_micros(&far, 7).is_ok());
    Ok(())
}

#[test]
fn legacy_datetimes_round_like_the_server() -> Result<(), CellError> {
    // 1970-01-01 00:00:00 plus two ticks prints as .007.
    let mut bytes = 25_567_i32.to_le_bytes().to_vec();
    bytes.extend(2_u32.to_le_bytes());
    assert_eq!(datetime_micros(&bytes)?, 7_000);
    let small = [0xDF, 0x63, 61, 0];
    assert_eq!(smalldatetime_micros(&small)?, 61 * 60_000_000);
    let mut text = String::new();
    write_timestamp(7_000, (0, 3), &mut text);
    assert_eq!(text, "1970-01-01 00:00:00.007");
    Ok(())
}

#[test]
fn offsets_print_local_time_with_the_offset() -> Result<(), CellError> {
    // 2024-01-01 00:00:00 UTC at +05:30 is 05:30 local.
    let days = 719_162 + 19_723;
    let mut bytes = vec![0, 0, 0];
    bytes.extend(date(days));
    bytes.extend(330_i16.to_le_bytes());
    assert_eq!(
        datetimeoffset_text(&bytes, 0)?,
        "2024-01-01 05:30:00 +05:30"
    );
    let mut west = vec![0, 0, 0];
    west.extend(date(days));
    west.extend((-60_i16).to_le_bytes());
    assert_eq!(datetimeoffset_text(&west, 0)?, "2023-12-31 23:00:00 -01:00");
    Ok(())
}
