// SPDX-License-Identifier: Apache-2.0
//! Round trip of every supported type: NULL, extremes, unicode, unknown types.
//!
//! Timestamps after 294247-01-10 (`i64::MAX` µs since 1970) exceed Arrow's
//! range although `PostgreSQL` accepts them; they fall back to tagged text.
use crate::support::{Collect, TestResult, column, connect, metadata, run, single_batch, strings};
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Date32Type, Decimal128Type, Float32Type, Float64Type, Int16Type, Int32Type, Int64Type,
    IntervalMonthDayNano, IntervalMonthDayNanoType, Time64MicrosecondType,
    TimestampMicrosecondType,
};
use arrow_array::{Array, RecordBatch};
use arrow_schema::{DataType, TimeUnit};

const MICROS_PER_DAY: i64 = 86_400_000_000;
const SOURCE: &str = "openrows.source_type";
const EXTENSION: &str = "ARROW:extension:name";
const FALLBACK: &str = "openrows.text_fallback";

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
    let iv = column(&batch, "iv")?;
    let iv = iv.as_primitive::<IntervalMonthDayNanoType>();
    let micros = ((4 * 60 + 5) * 60 + 6) * 1_000_000 + 789_000;
    assert_eq!(
        iv.values()[1..],
        [
            IntervalMonthDayNano::new(14, 3, micros * 1_000),
            IntervalMonthDayNano::new(0, 0, -1_000)
        ]
    );
    assert_eq!(strings(&batch, "ip")?, ["192.168.0.1/24", "::1"]);
    let arr = column(&batch, "arr")?;
    let arr = arr.as_list::<i32>();
    let first = arr.value(1);
    let first: Vec<_> = first.as_primitive::<Int32Type>().iter().collect();
    assert_eq!(
        (first, arr.value(2).len()),
        (vec![Some(1), None, Some(3)], 0)
    );
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
    assert_eq!(metadata(batch, "ip", EXTENSION)?, "openrows.inet");
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn times_networks_and_ranges_decode_natively() -> TestResult {
    let sql = "SELECT * FROM (VALUES
        (NULL::time, NULL::timetz, NULL::cidr, NULL::macaddr, NULL::macaddr8,
         NULL::int4range, NULL::datemultirange),
        ('13:05:00.25'::time, '13:05:00+05:30'::timetz, '10.1.0.0/16'::cidr,
         '08:00:2b:01:02:03'::macaddr, '08:00:2b:01:02:03:04:05'::macaddr8,
         '[1,10)'::int4range, '{[2024-01-01,2024-02-01)}'::datemultirange),
        ('24:00:00'::time, '00:00:00-01'::timetz, '::/0'::cidr,
         'ff:ff:ff:ff:ff:ff'::macaddr, '00:00:00:00:00:00:00:00'::macaddr8,
         'empty'::int4range, '{}'::datemultirange)
        ) AS t(tm, tz, net, mac, mac8, r, mr)";
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    // 24:00:00 has no Time64 value, so the column is tagged text.
    assert_eq!(column(&batch, "tm")?.data_type(), &DataType::Utf8);
    assert_eq!(metadata(&batch, "tm", FALLBACK)?, "true");
    assert_eq!(strings(&batch, "tm")?, ["13:05:00.25", "24:00:00"]);
    assert_eq!(strings(&batch, "tz")?, ["13:05:00+05:30", "00:00:00-01"]);
    assert_eq!(strings(&batch, "net")?, ["10.1.0.0/16", "::/0"]);
    assert_eq!(
        strings(&batch, "mac")?,
        ["08:00:2b:01:02:03", "ff:ff:ff:ff:ff:ff"]
    );
    assert_eq!(
        strings(&batch, "mac8")?,
        ["08:00:2b:01:02:03:04:05", "00:00:00:00:00:00:00:00"]
    );
    assert_eq!(strings(&batch, "r")?, ["[1,10)", "empty"]);
    assert_eq!(strings(&batch, "mr")?, ["{[2024-01-01,2024-02-01)}", "{}"]);
    for (name, extension) in [("mac", "openrows.macaddr"), ("r", "openrows.range")] {
        assert_eq!(metadata(&batch, name, EXTENSION)?, extension);
    }
    let time = single_batch(&mut connect().await?, "SELECT '13:05:00.25'::time AS tm").await?;
    let time = column(&time, "tm")?;
    assert_eq!(time.data_type(), &DataType::Time64(TimeUnit::Microsecond));
    let micros = time.as_primitive::<Time64MicrosecondType>().value(0);
    assert_eq!(micros, (13 * 3_600 + 5 * 60) * 1_000_000 + 250_000);
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn arrays_are_lists_until_a_value_needs_array_text() -> TestResult {
    let sql = "SELECT * FROM (VALUES
        (NULL::text[], NULL::int4[], NULL::interval[]),
        (ARRAY['a b', NULL, '']::text[], '{1,2}'::int4[], '{1 day}'::interval[]),
        (ARRAY['x']::text[], '{{1,2},{3,4}}'::int4[], '{-2 mons}'::interval[])
        ) AS t(txt, ints, ivs)";
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    let txt = column(&batch, "txt")?;
    let txt = txt.as_list::<i32>();
    let first = txt.value(1);
    let first: Vec<_> = first.as_string::<i32>().iter().collect();
    assert_eq!(first, [Some("a b"), None, Some("")]);
    assert_eq!(metadata(&batch, "txt", SOURCE)?, "text[]");
    // The two-dimensional value switches the column to PostgreSQL's array text.
    assert_eq!(metadata(&batch, "ints", FALLBACK)?, "true");
    assert_eq!(strings(&batch, "ints")?, ["{1,2}", "{{1,2},{3,4}}"]);
    let ivs = column(&batch, "ivs")?;
    let ivs = ivs.as_list::<i32>().value(2);
    let ivs = ivs.as_primitive::<IntervalMonthDayNanoType>();
    assert_eq!(ivs.value(0), IntervalMonthDayNano::new(-2, 0, 0));
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn out_of_range_values_fall_back_to_tagged_text() -> TestResult {
    let sql = "SELECT * FROM (VALUES
        (NULL::timestamp, NULL::timestamptz, NULL::date, NULL::numeric(10,2), NULL::int4),
        ('infinity'::timestamp, '-infinity'::timestamptz, 'infinity'::date, 'NaN'::numeric(10,2), 1),
        ('294276-12-31 23:59:59.999999'::timestamp, '2024-05-06 07:08:09.5+00'::timestamptz,
         '0044-03-15 BC'::date, 12.5::numeric(10,2), 2)
        ) AS t(ts, tz, d, n, i)";
    let batch = single_batch(&mut connect().await?, sql).await?;
    assert_first_row_null(&batch);
    let expected = [
        (
            "ts",
            "timestamp",
            ["infinity", "294276-12-31 23:59:59.999999"],
        ),
        (
            "tz",
            "timestamptz",
            ["-infinity", "2024-05-06 07:08:09.5+00"],
        ),
        ("d", "date", ["infinity", "0044-03-15 BC"]),
        ("n", "numeric", ["NaN", "12.50"]),
    ];
    for (name, source, values) in expected {
        assert_eq!(strings(&batch, name)?, values, "{name}");
        assert_eq!(metadata(&batch, name, SOURCE)?, source, "{name}");
        assert_eq!(metadata(&batch, name, FALLBACK)?, "true", "{name}");
    }
    let i = column(&batch, "i")?;
    assert_eq!(i.as_primitive::<Int32Type>().values()[1..], [1, 2]);
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn a_late_fallback_continues_in_a_new_result() -> TestResult {
    let sql = "SELECT CASE WHEN g = 300 THEN 'infinity'::date ELSE DATE '2000-01-01' + g END AS d
        FROM generate_series(1, 300) AS g";
    let mut session = connect().await?;
    let (summary, sink) = run(&mut session, sql, Collect::default()).await;
    assert_eq!(summary?.rows, 300);
    assert_eq!((sink.results, sink.notices.len()), (2, 1));
    let [typed, text] = <[RecordBatch; 2]>::try_from(sink.batches)
        .map_err(|batches| format!("expected two batches, got {}", batches.len()))?;
    assert_eq!(typed.column(0).data_type(), &DataType::Date32);
    assert_eq!(text.column(0).data_type(), &DataType::Utf8);
    let values = text.column(0).as_string::<i32>();
    assert_eq!(
        (values.value(0), values.value(99)),
        ("2000-07-20", "infinity")
    );
    assert_eq!(sink.schema.as_ref(), Some(&text.schema()));
    Ok(())
}
