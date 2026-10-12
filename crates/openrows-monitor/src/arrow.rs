// SPDX-License-Identifier: Apache-2.0
//! Bounded binary Arrow windows; source rows never use JSON IPC.
use crate::{LiveStore, MetricKind, MonitorError, SampleState, Source};
use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;

pub(crate) fn window(
    store: &LiveStore,
    options: crate::WindowOptions,
) -> Result<Vec<u8>, MonitorError> {
    // Four collectors: 8 MiB retained + 16 MiB in-flight + one 4 MiB IPC workspace < 32 MiB.
    static WINDOW: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _window = WINDOW
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let batch = match options.source {
        Some(source) => table(store, source, options)?,
        None => series(store, options)?,
    };
    encode(&batch)
}

fn table(
    store: &LiveStore,
    source: Source,
    options: crate::WindowOptions,
) -> Result<RecordBatch, MonitorError> {
    let reading = store.tables.get(&source).ok_or(MonitorError::Stopped)?;
    let projected = (source == Source::Statements).then(|| store.statement_rankings(reading));
    let reading = projected.as_ref().unwrap_or(reading);
    let mut rows: Vec<_> = reading.rows.iter().collect();
    if source == Source::Statements {
        rows.sort_by(|a, b| {
            rank(reading, b, options.mean).total_cmp(&rank(reading, a, options.mean))
        });
    }
    rows.drain(..(options.row_start as usize).min(rows.len()));
    if source == Source::Statements {
        rows.truncate(options.row_count.clamp(1, 100) as usize);
    } else {
        rows.truncate(options.row_count.min(1_000) as usize);
    }
    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();
    for (index, column) in reading.columns.iter().enumerate() {
        let numeric = matches!(column.kind, MetricKind::Counter | MetricKind::Gauge);
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("unit".to_owned(), column.unit.clone());
        metadata.insert("semantics".to_owned(), format!("{:?}", column.kind));
        fields.push(
            Field::new(
                &column.name,
                if numeric {
                    DataType::Float64
                } else {
                    DataType::Utf8
                },
                true,
            )
            .with_metadata(metadata),
        );
        if numeric {
            arrays.push(Arc::new(Float64Array::from(
                rows.iter()
                    .map(|row| {
                        row.get(index)
                            .and_then(Option::as_deref)
                            .and_then(|s| s.parse::<f64>().ok())
                            .filter(|n| n.is_finite())
                    })
                    .collect::<Vec<_>>(),
            )));
        } else {
            arrays.push(Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| row.get(index).and_then(Option::as_deref))
                    .collect::<Vec<_>>(),
            )));
        }
    }
    let schema = Arc::new(
        Schema::new(fields).with_metadata(std::collections::HashMap::from([
            (
                "sampleState".to_owned(),
                state_name(&reading.state).to_owned(),
            ),
            ("epoch".to_owned(), reading.epoch.clone()),
            ("partial".to_owned(), reading.partial.to_string()),
        ])),
    );
    if arrays.is_empty() {
        return Ok(RecordBatch::new_empty(schema));
    }
    RecordBatch::try_new(schema, arrays).map_err(|_| MonitorError::Encoding)
}

fn rank(reading: &crate::Reading, row: &[Option<String>], mean: bool) -> f64 {
    crate::advisor::catalog::cell(
        reading,
        row,
        if mean {
            "live_window_mean_ms"
        } else {
            "live_window_total_ms"
        },
    )
    .and_then(|value| value.parse::<f64>().ok())
    .unwrap_or(0.0)
}

fn series(store: &LiveStore, options: crate::WindowOptions) -> Result<RecordBatch, MonitorError> {
    let mut keys = Vec::new();
    let mut times = Vec::new();
    let mut values = Vec::new();
    let mut states = Vec::new();
    let mut units = Vec::new();
    for (key, point) in store
        .series
        .iter()
        .flat_map(|(key, points)| points.iter().map(move |point| (key, point)))
        .skip(options.row_start as usize)
        .take(options.row_count.min(1_000) as usize)
    {
        keys.push(key.as_str());
        times.push(point.wall_ms);
        values.push(point.value);
        states.push(state_name(&point.state));
        units.push(point.unit.as_str());
    }
    let schema = Schema::new(vec![
        Field::new("metric", DataType::Utf8, false),
        Field::new("collected_ms", DataType::Float64, false),
        Field::new("value", DataType::Float64, true),
        Field::new("state", DataType::Utf8, false),
        Field::new("unit", DataType::Utf8, false),
    ]);
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(StringArray::from(keys)),
            Arc::new(Float64Array::from(times)),
            Arc::new(Float64Array::from(values)),
            Arc::new(StringArray::from(states)),
            Arc::new(StringArray::from(units)),
        ],
    )
    .map_err(|_| MonitorError::Encoding)
}

pub(crate) fn state_name(state: &SampleState) -> &'static str {
    match state {
        SampleState::Ok => "ok",
        SampleState::Unknown => "unknown",
        SampleState::Unavailable(_) => "unavailable",
        SampleState::PermissionDenied(_) => "permissionDenied",
        SampleState::Gap => "gap",
        SampleState::CounterReset => "counterReset",
    }
}

fn encode(batch: &RecordBatch) -> Result<Vec<u8>, MonitorError> {
    let mut bytes = Vec::new();
    let mut writer = StreamWriter::try_new(&mut bytes, batch.schema().as_ref())
        .map_err(|_| MonitorError::Encoding)?;
    writer.write(batch).map_err(|_| MonitorError::Encoding)?;
    writer.finish().map_err(|_| MonitorError::Encoding)?;
    drop(writer);
    if bytes.len() > 1_024 * 1_024 {
        return Err(MonitorError::Limit);
    }
    Ok(bytes)
}
