// SPDX-License-Identifier: Apache-2.0
use super::*;

fn rule() -> AlertRule {
    AlertRule {
        id: "waiting".into(),
        title: "Waiting sessions".into(),
        metric: "sessions:waiting".into(),
        comparison: Comparison::Above,
        fire: 5.0,
        clear: 1.0,
        fire_after: Hold {
            samples: 3,
            ms: 10_000,
        },
        clear_after: Hold {
            samples: 2,
            ms: 5_000,
        },
        cooldown_ms: 0,
        severity: Severity::Warning,
    }
}

fn engine(rule: AlertRule) -> Result<AlertEngine, RuleError> {
    AlertEngine::new(vec![rule])
}

fn sample(seconds: u32, value: Option<f64>) -> Observation {
    Observation {
        metric: "sessions:waiting".into(),
        scope: Scope::Server,
        wall_ms: f64::from(seconds) * 1_000.0,
        value,
        state: if value.is_some() {
            SampleState::Ok
        } else {
            SampleState::Unknown
        },
    }
}

/// Feed `(second, value)` pairs and collect transition kinds as `o`/`r` markers.
fn run(engine: &mut AlertEngine, samples: &[(u32, Option<f64>)]) -> String {
    samples
        .iter()
        .map(|(seconds, value)| {
            engine
                .observe(&sample(*seconds, *value))
                .iter()
                .map(|transition| match transition {
                    Transition::Opened(_) => 'o',
                    Transition::Resolved(_) => 'r',
                })
                .collect::<String>()
                .chars()
                .next()
                .unwrap_or('.')
        })
        .collect()
}

fn open(engine: &mut AlertEngine) -> String {
    run(engine, &[(0, Some(9.0)), (5, Some(9.0)), (10, Some(9.0))])
}

#[test]
fn threshold_and_duration_require_both_samples_and_time() -> Result<(), RuleError> {
    let mut alerts = engine(rule())?;
    assert_eq!(run(&mut alerts, &[(0, Some(4.9)), (5, Some(4.0))]), "..");
    // Three samples but only 4 s held; then the fourth sample satisfies 10 s.
    assert_eq!(
        run(
            &mut alerts,
            &[
                (20, Some(5.0)),
                (22, Some(6.0)),
                (24, Some(7.0)),
                (30, Some(8.0))
            ]
        ),
        "...o"
    );
    let incidents = alerts.incidents();
    assert_eq!(incidents.len(), 1);
    let incident = &incidents[0];
    assert!((incident.started_ms - 20_000.0).abs() < f64::EPSILON);
    assert_eq!(incident.onset.len(), 4);
    assert_eq!(incident.ended_ms, None);
    // A breach interrupted by a healthy value restarts the duration.
    let mut alerts = engine(rule())?;
    assert_eq!(
        run(
            &mut alerts,
            &[
                (0, Some(9.0)),
                (5, Some(9.0)),
                (6, Some(2.0)),
                (10, Some(9.0))
            ]
        ),
        "...."
    );
    Ok(())
}

#[test]
fn hysteresis_band_neither_resolves_nor_reopens() -> Result<(), RuleError> {
    let mut alerts = engine(rule())?;
    assert_eq!(open(&mut alerts), "..o");
    // 3 is below the firing threshold but above the recovery threshold.
    assert_eq!(
        run(
            &mut alerts,
            &[(15, Some(3.0)), (20, Some(3.0)), (40, Some(3.0))]
        ),
        "..."
    );
    assert_eq!(alerts.incidents()[0].ended_ms, None);
    // Recovery samples interrupted by the band restart the recovery hold.
    assert_eq!(
        run(
            &mut alerts,
            &[
                (45, Some(1.0)),
                (48, Some(4.0)),
                (50, Some(1.0)),
                (55, Some(0.0))
            ]
        ),
        "...r"
    );
    Ok(())
}

#[test]
fn recovery_records_end_peak_and_last_evidence() -> Result<(), RuleError> {
    let mut alerts = engine(rule())?;
    open(&mut alerts);
    assert_eq!(
        run(
            &mut alerts,
            &[(15, Some(12.0)), (20, Some(1.0)), (25, Some(0.0))]
        ),
        "..r"
    );
    let incident = &alerts.incidents()[0];
    assert_eq!(incident.ended_ms, Some(25_000.0));
    assert!((incident.peak.value - 12.0).abs() < f64::EPSILON);
    assert!(incident.last.value.abs() < f64::EPSILON);
    assert_eq!(incident.rule.id, "waiting");
    Ok(())
}

#[test]
fn one_open_incident_per_rule_and_scope_with_cooldown() -> Result<(), RuleError> {
    let mut alerts = engine(AlertRule {
        cooldown_ms: 60_000,
        ..rule()
    })?;
    open(&mut alerts);
    // Continued breaching never duplicates the open incident.
    assert_eq!(
        run(
            &mut alerts,
            &[(15, Some(9.0)), (20, Some(9.0)), (30, Some(9.0))]
        ),
        "..."
    );
    // A different scope is tracked independently.
    let mut database = sample(31, Some(9.0));
    database.scope = Scope::Database;
    assert!(alerts.observe(&database).is_empty());
    assert_eq!(run(&mut alerts, &[(35, Some(0.0)), (40, Some(0.0))]), ".r");
    // Breaching again inside the cooldown waits; afterwards one new incident opens.
    assert_eq!(
        run(
            &mut alerts,
            &[
                (50, Some(9.0)),
                (60, Some(9.0)),
                (90, Some(9.0)),
                (100, Some(9.0))
            ]
        ),
        "...o"
    );
    assert_eq!(alerts.incidents().len(), 2);
    // Repeated or out-of-order timestamps are ignored.
    assert_eq!(run(&mut alerts, &[(100, Some(0.0)), (99, Some(0.0))]), "..");
    Ok(())
}

#[test]
fn unknown_samples_never_fire_or_clear() -> Result<(), RuleError> {
    let mut alerts = engine(rule())?;
    // Unknown interrupts a breach streak: the duration restarts afterwards.
    assert_eq!(
        run(
            &mut alerts,
            &[
                (0, Some(9.0)),
                (5, Some(9.0)),
                (8, None),
                (10, Some(9.0)),
                (15, Some(9.0))
            ]
        ),
        "....."
    );
    let mut alerts = engine(rule())?;
    assert_eq!(
        run(&mut alerts, &[(0, None), (5, None), (10, None), (30, None)]),
        "...."
    );
    let mut alerts = engine(rule())?;
    open(&mut alerts);
    // Unknown, gaps, resets and non-finite values keep the incident open.
    let mut states = vec![sample(15, None)];
    for (seconds, state) in [
        (20, SampleState::Gap),
        (25, SampleState::CounterReset),
        (30, SampleState::PermissionDenied("role".into())),
    ] {
        let mut observation = sample(seconds, Some(0.0));
        observation.state = state;
        states.push(observation);
    }
    states.push(sample(35, Some(f64::NAN)));
    for observation in &states {
        assert!(alerts.observe(observation).is_empty());
    }
    assert_eq!(alerts.incidents()[0].ended_ms, None);
    // Unknown between recovery samples restarts the recovery hold.
    assert_eq!(
        run(
            &mut alerts,
            &[
                (40, Some(0.0)),
                (42, None),
                (45, Some(0.0)),
                (50, Some(0.0))
            ]
        ),
        "...r"
    );
    Ok(())
}

#[test]
fn rules_are_validated_and_defaults_are_valid() {
    let invalid = [
        AlertRule {
            clear: 6.0,
            ..rule()
        },
        AlertRule {
            comparison: Comparison::Below,
            ..rule()
        },
        AlertRule {
            fire: f64::INFINITY,
            ..rule()
        },
        AlertRule {
            metric: String::new(),
            ..rule()
        },
        AlertRule {
            fire_after: Hold { samples: 0, ms: 0 },
            ..rule()
        },
    ];
    let errors: Vec<_> = invalid
        .into_iter()
        .map(|rule| rule.validate().err())
        .collect();
    assert_eq!(
        errors,
        [
            Some(RuleError::Hysteresis),
            Some(RuleError::Hysteresis),
            Some(RuleError::NonFinite),
            Some(RuleError::Empty),
            Some(RuleError::Hold),
        ]
    );
    assert_eq!(
        AlertEngine::new(vec![rule(), rule()]).err(),
        Some(RuleError::Duplicate)
    );
    assert!(AlertEngine::new(default_rules()).is_ok());
}

#[test]
fn incident_list_is_bounded_and_evicts_resolved_first() -> Result<(), RuleError> {
    let mut alerts = engine(AlertRule {
        fire_after: Hold { samples: 1, ms: 0 },
        clear_after: Hold { samples: 1, ms: 0 },
        ..rule()
    })?;
    let samples: Vec<_> = (0..120)
        .map(|second| (second, Some(if second % 2 == 0 { 9.0 } else { 0.0 })))
        .collect();
    run(&mut alerts, &samples);
    let incidents = alerts.incidents();
    assert_eq!(incidents.len(), 50);
    assert_eq!(incidents[0].id, 60);
    Ok(())
}
