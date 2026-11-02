# SPDX-License-Identifier: Apache-2.0
"""Validate M1-21 completion popup reports; gate popup latency p95 per trigger kind."""
from bench_ipc_metrics import valid_timing
from bench_metrics import BenchmarkError, percentile, summary

TRIGGERS = (("keyword", "S"), ("table", "o"), ("schemaObject", "."), ("column", "."))
REPEATS = 25
OBJECTS = 50_000
BUDGET_MS = 50
PHASES = ("openMs", "paintMs", "popupMs")
EPSILON = 1e-6


def expected_steps():
    """Every repeat measures each trigger kind once, in script order."""
    return [trigger for _ in range(REPEATS) for trigger in TRIGGERS]


def validate_sample(sample, index, step):
    if not isinstance(sample, dict) or sample.get("index") != index:
        raise BenchmarkError("Completion samples are missing, reordered or duplicated")
    if (sample.get("kind"), sample.get("key")) != step:
        raise BenchmarkError(f"Unexpected completion trigger at sample {index}")
    if type(sample.get("options")) is not int or sample["options"] < 1:
        raise BenchmarkError(f"Empty completion popup at sample {index}")
    if not all(valid_timing(sample.get(phase)) for phase in PHASES):
        raise BenchmarkError(f"Invalid completion timing at sample {index}")
    if abs(sample["openMs"] + sample["paintMs"] - sample["popupMs"]) > EPSILON:
        raise BenchmarkError(f"Completion phases do not add up at sample {index}")


def validate_run(run):
    if not isinstance(run, dict) or run.get("objects") != OBJECTS:
        raise BenchmarkError("Missing completion run or not the 50 000-object catalog")
    if not valid_timing(run.get("activateDelayMs")):
        raise BenchmarkError("Missing completion activation delay")
    if not valid_timing(run.get("refreshIntervalMs")) or run["refreshIntervalMs"] == 0:
        raise BenchmarkError("Invalid completion refreshIntervalMs")
    samples, steps = run.get("samples"), expected_steps()
    if not isinstance(samples, list) or len(samples) != len(steps):
        raise BenchmarkError(f"Expected {len(steps)} completion popups")
    for index, (sample, step) in enumerate(zip(samples, steps)):
        validate_sample(sample, index, step)
    return samples


def completion_metrics(report):
    """Popup latency summary per trigger kind, with phase p95 and the configured delay."""
    run = report.get("completion") if isinstance(report, dict) else None
    samples = validate_run(run)
    metrics = {}
    for kind, _ in TRIGGERS:
        chosen = [sample for sample in samples if sample["kind"] == kind]
        values = [sample["popupMs"] for sample in chosen]
        metrics[kind] = dict(summary(values), samples=len(chosen),
                             over_budget=sum(value > BUDGET_MS for value in values),
                             options_min=min(sample["options"] for sample in chosen),
                             phase_p95_ms={phase: percentile([s[phase] for s in chosen], 0.95)
                                           for phase in PHASES})
    return dict(kinds=metrics, activate_delay_ms=run["activateDelayMs"],
                refresh_interval_ms=run["refreshIntervalMs"])


def completion_budgets(metrics):
    """Exactly 50 ms passes, per trigger kind."""
    kinds = metrics.get("kinds") if isinstance(metrics, dict) else None
    if not isinstance(kinds, dict) or set(kinds) != {kind for kind, _ in TRIGGERS}:
        raise BenchmarkError("Missing completion trigger metrics")
    return [dict(metric=f"{kind} completion popup p95", actual=kinds[kind]["p95"],
                 limit=BUDGET_MS, unit="ms", passed=kinds[kind]["p95"] <= BUDGET_MS)
            for kind, _ in TRIGGERS]
