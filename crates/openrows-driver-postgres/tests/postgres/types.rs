// SPDX-License-Identifier: Apache-2.0
//! Round trip of every supported type: NULL, extremes, unicode, unknown types.
//!
//! Timestamps after 294247-01-10 (`i64::MAX` µs since 1970) exceed Arrow's
//! range although `PostgreSQL` accepts them; see the error test.
use crate::support::{TestResult, column, connect, metadata, single_batch, strings};
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Float32Type, Float64Type, Int16Type, Int32Type, Int64Type,
    TimestampMicrosecondType,
};
use arrow_array::{Array, RecordBatch};
use arrow_schema::{DataType, TimeUnit};

const MICROS_PER_DAY: i64 = 86_400_000_000;
const SOURCE: &str = "openrows.source_type";
const EXTENSION: &str = "ARROW:extension:name";

fn assert_first_row_null(batch: &RecordBatch) {
    assert!(
        batch.columns().iter().all(|a| a.is_null(0)),
        "row 0 is NULL everywhere"
    );
}

/// Days since 1970-01-01, proleptic Gregorian calendar, astronomical years.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn numbers_and_booleans_keep_nulls_and_extremes() -> TestResult {
    let sql = "SELECT * FROM (VALUES
        (NULL::int2, NULL::int4, NULL::int8, NULL::float4, NULL::float8, NULL::bool),
        ((-32768)::int2, (-2147483648)::int4, (-9223372036854775808)::int8,
         '-Infinity'::float4, '-1.7976931348623157e308'::float8, false),
        (32767::int2, 2147483647::int4, 9223372036854775807::int8,
         'NaN'::float4, '4.9e-324'::float8, true)) AS t(i2, i4, i8, f4, f8, b)";
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    let i2 = column(&batch, "i2")?;
    assert_eq!(
        i2.as_primitive::<Int16Type>().values()[1..],
        [i16::MIN, i16::MAX]
    );
    let i4 = column(&batch, "i4")?;
    assert_eq!(
        i4.as_primitive::<Int32Type>().values()[1..],
        [i32::MIN, i32::MAX]
    );
    let i8 = column(&batch, "i8")?;
    assert_eq!(
        i8.as_primitive::<Int64Type>().values()[1..],
        [i64::MIN, i64::MAX]
    );
    let f4 = column(&batch, "f4")?;
    let f4 = f4.as_primitive::<Float32Type>();
    assert!(f4.value(1) == f32::NEG_INFINITY && f4.value(2).is_nan());
    let f8 = column(&batch, "f8")?;
    assert_eq!(
        f8.as_primitive::<Float64Type>().values()[1..],
        [f64::MIN, 4.9e-324]
    );
    let b = column(&batch, "b")?;
    assert!(!b.as_boolean().value(1) && b.as_boolean().value(2));
    assert_eq!(metadata(&batch, "i8", SOURCE)?, "int8");
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn text_binary_and_json_keep_unicode_and_empty_values() -> TestResult {
    let sql = r#"SELECT * FROM (VALUES
        (NULL::text, NULL::varchar(5), NULL::char(3), NULL::name, NULL::bytea, NULL::json, NULL::jsonb),
        (''::text, ''::varchar(5), ''::char(3), ''::name, ''::bytea, '[]'::json, '{}'::jsonb),
        ('héllo 世界 👩‍💻 é'::text, 'ab😀'::varchar(5), 'x'::char(3), 'pg_name'::name,
         '\x00ff00'::bytea, '{"k": "ü"}'::json, '{"b": [1, 2], "a": null}'::jsonb)
        ) AS t(tx, vc, bp, nm, by, js, jb)"#;
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    assert_eq!(strings(&batch, "tx")?, ["", "héllo 世界 👩‍💻 é"]);
    assert_eq!(strings(&batch, "vc")?, ["", "ab😀"]);
    assert_eq!(strings(&batch, "bp")?, ["   ", "x  "]);
    assert_eq!(strings(&batch, "nm")?, ["", "pg_name"]);
    assert_eq!(strings(&batch, "js")?, ["[]", r#"{"k": "ü"}"#]);
    assert_eq!(
        strings(&batch, "jb")?,
        ["{}", r#"{"a": null, "b": [1, 2]}"#]
    );
    let by = column(&batch, "by")?;
    let by = by.as_binary::<i32>();
    assert_eq!((by.value(1), by.value(2)), (&b""[..], &[0, 255, 0][..]));
    assert_eq!(metadata(&batch, "jb", EXTENSION)?, "openrows.json");
    assert_eq!(metadata(&batch, "bp", SOURCE)?, "bpchar");
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn temporal_values_cover_the_postgres_range() -> TestResult {
    let sql = "SELECT * FROM (VALUES
        (NULL::timestamp, NULL::timestamptz, NULL::date),
        ('4713-11-24 00:00:00 BC'::timestamp, '4713-11-24 00:00:00+00 BC'::timestamptz,
         '4713-11-24 BC'::date),
        ('294247-01-01 00:00:00'::timestamp, '294247-01-01 00:00:00+00'::timestamptz,
         '5874897-12-31'::date),
        ('1970-01-01'::timestamp, '1969-12-31 23:59:59.999999+00'::timestamptz, '1970-01-01'::date)
        ) AS t(ts, tz, d)";
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    let min_day = days_from_civil(-4712, 11, 24);
    let (min, max) = (
        min_day * MICROS_PER_DAY,
        days_from_civil(294_247, 1, 1) * MICROS_PER_DAY,
    );
    let ts = column(&batch, "ts")?;
    assert_eq!(
        ts.as_primitive::<TimestampMicrosecondType>().values()[1..],
        [min, max, 0]
    );
    let tz = column(&batch, "tz")?;
    let utc = DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()));
    assert_eq!(tz.data_type(), &utc);
    assert_eq!(
        tz.as_primitive::<TimestampMicrosecondType>().values()[1..],
        [min, max, -1]
    );
    let d = column(&batch, "d")?;
    let days: Vec<_> = d.as_primitive::<Date32Type>().values()[1..]
        .iter()
        .map(|&v| i64::from(v))
        .collect();
    assert_eq!(days, [min_day, days_from_civil(5_874_897, 12, 31), 0]);
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn uuid_numeric_and_unknown_types_keep_exact_values() -> TestResult {
    let sql = "SELECT * FROM (VALUES
        (NULL::uuid, NULL::numeric(38,10), NULL::numeric, NULL::interval, NULL::inet,
         NULL::int4[], NULL::point),
        ('00000000-0000-0000-0000-000000000000'::uuid,
         '-9999999999999999999999999999.9999999999'::numeric(38,10),
         '-123456789012345678901234567890123456789.000000000000000000001'::numeric,
         '1 year 2 mons 3 days 04:05:06.789'::interval, '192.168.0.1/24'::inet,
         '{1,NULL,3}'::int4[], '(1.5,-2)'::point),
        ('ffffffff-ffff-ffff-ffff-fffffffffffe'::uuid, '0.0000000001'::numeric(38,10),
         'NaN'::numeric, '-00:00:00.000001'::interval, '::1'::inet, '{}'::int4[], '(0,0)'::point)
        ) AS t(u, dec, num, iv, ip, arr, pt)";
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    let u = column(&batch, "u")?;
    let u = u.as_fixed_size_binary();
    assert_eq!((u.value(1), u.value(2)[15]), (&[0_u8; 16][..], 0xfe));
    let dec = column(&batch, "dec")?;
    assert_eq!(dec.data_type(), &DataType::Decimal128(38, 10));
    assert_eq!(
        dec.as_primitive::<Decimal128Type>().values()[1..],
        [1 - 10_i128.pow(38), 1]
    );
    let exact = "-123456789012345678901234567890123456789.000000000000000000001";
    assert_eq!(strings(&batch, "num")?, [exact, "NaN"]);
    assert_eq!(
        strings(&batch, "iv")?,
        ["1 year 2 mons 3 days 04:05:06.789", "-00:00:00.000001"]
    );
    assert_eq!(strings(&batch, "ip")?, ["192.168.0.1/24", "::1"]);
    assert_eq!(strings(&batch, "arr")?, ["{1,NULL,3}", "{}"]);
    assert_eq!(strings(&batch, "pt")?, ["(1.5,-2)", "(0,0)"]);
    assert_source_types(&batch)
}

fn assert_source_types(batch: &RecordBatch) -> TestResult {
    let expected = [
        ("u", "uuid"),
        ("dec", "numeric"),
        ("num", "numeric"),
        ("iv", "interval"),
        ("ip", "inet"),
        ("arr", "integer[]"),
        ("pt", "point"),
    ];
    for (name, source) in expected {
        assert_eq!(metadata(batch, name, SOURCE)?, source, "{name}");
    }
    assert_eq!(metadata(batch, "u", EXTENSION)?, "arrow.uuid");
    assert_eq!(metadata(batch, "num", EXTENSION)?, "openrows.decimal");
    Ok(())
}
