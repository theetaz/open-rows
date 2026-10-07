# SPDX-License-Identifier: Apache-2.0
"""Validate M0-S5 editor keystroke reports; gate keystroke-to-paint p95 per configuration."""
from bench_ipc_metrics import timer_tolerance, valid_timing
from bench_metrics import BenchmarkError, percentile, summary

CONFIGS = ("sql", "vim")
POSITIONS = ("top", "middle", "end")
INSERTS, DELETES = 60, 20
LINES = 5_000
BUDGET_MS = 16
DELETE_KEYS = dict(sql="Backspace", vim="x")
PHASES = ("queueMs", "inputMs", "vsyncWaitMs", "frameMs", "wallMs", "keystrokeToPaintMs")
EPSILON = 1e-6


def expected_steps():
    """Per edit site: sixty inserts, then twenty deletes, sites in script order."""
    return [(position, kind) for position in POSITIONS
            for kind in ("insert",) * INSERTS + ("delete",) * DELETES]


def validate_sample(sample, index, step, config):
    if not isinstance(sample, dict) or sample.get("index") != index:
        raise BenchmarkError("Editor samples are missing, reordered or duplicated")
    if (sample.get("position"), sample.get("kind")) != step:
        raise BenchmarkError(f"Unexpected editor keystroke at sample {index}")
    key = sample.get("key")
    typed = isinstance(key, str) and len(key) == 1
    if not (key == DELETE_KEYS[config] if step[1] == "delete" else typed):
        raise BenchmarkError(f"Wrong {step[1]} key for {config} at sample {index}")
    if not all(valid_timing(sample.get(phase)) for phase in PHASES):
        raise BenchmarkError(f"Invalid editor timing at sample {index}")
    gated = sample["queueMs"] + sample["inputMs"] + sample["frameMs"]
    wall = sample["inputMs"] + sample["vsyncWaitMs"] + sample["frameMs"]
    if abs(gated - sample["keystrokeToPaintMs"]) > EPSILON or abs(wall - sample["wallMs"]) > EPSILON:
        raise BenchmarkError(f"Editor phases do not add up at sample {index}")


def validate_run(run, config):
    if not isinstance(run, dict) or run.get("config") != config:
        raise BenchmarkError("Expected the sql then vim editor configurations")
    if run.get("lines") != LINES:
        raise BenchmarkError("Editor document is not 5000 lines")
    if type(run.get("documentChars")) is not int or run["documentChars"] <= 0:
        raise BenchmarkError("Missing editor document size")
    for key in ("refreshIntervalMs", "durationMs"):
        if not valid_timing(run.get(key)) or run[key] == 0:
            raise BenchmarkError(f"Invalid editor {key}")
    samples, steps = run.get("samples"), expected_steps()
    if not isinstance(samples, list) or len(samples) != len(steps):
        raise BenchmarkError(f"Expected {len(steps)} {config} keystrokes")
    for index, (sample, step) in enumerate(zip(samples, steps)):
        validate_sample(sample, index, step, config)
    return samples


def phase_p95(samples):
    return {phase: percentile([sample[phase] for sample in samples], 0.95) for phase in PHASES}


def run_metrics(run, config):
    samples = validate_run(run, config)
    values = [sample["keystrokeToPaintMs"] for sample in samples]
    positions = {position: phase_p95([s for s in samples if s["position"] == position])
                 for position in POSITIONS}
    return dict(summary(values), samples=len(samples), over_budget=sum(value > BUDGET_MS for value in values),
                phase_p95_ms=phase_p95(samples), position_p95_ms=positions,
                refresh_interval_ms=run["refreshIntervalMs"], duration_ms=run["durationMs"],
                document_chars=run["documentChars"])


def editor_metrics(report, pid):
    if not isinstance(report, dict) or report.get("error") is not None:
        raise BenchmarkError("Editor spike failed or returned an invalid report")
    if (report.get("schema_version"), report.get("pid"), report.get("mode")) != (1, pid, "editor"):
        raise BenchmarkError("Unexpected editor schema/PID/mode")
    timer_tolerance(report)
    runs = report.get("editor")
    if not isinstance(runs, list) or len(runs) != len(CONFIGS):
        raise BenchmarkError("Expected both editor configurations")
    return {config: run_metrics(run, config) for config, run in zip(CONFIGS, runs)}


def editor_budgets(metrics):
    """Exactly 16 ms passes; no timer-quantisation allowance."""
    if set(metrics) != set(CONFIGS):
        raise BenchmarkError("Missing editor configuration metrics")
    return [dict(metric=f"{config} keystroke-to-paint p95", actual=metrics[config]["p95"],
                 limit=BUDGET_MS, unit="ms", passed=metrics[config]["p95"] <= BUDGET_MS)
            for config in CONFIGS]


def longest_keystrokes(report, count=10):
    """Slowest gated keystrokes across configurations, with their phases."""
    runs = report.get("editor") if isinstance(report, dict) else None
    samples = [dict(sample, config=run.get("config")) for run in (runs if isinstance(runs, list) else [])
               if isinstance(run, dict) for sample in run.get("samples") or []
               if isinstance(sample, dict) and valid_timing(sample.get("keystrokeToPaintMs"))]
    return sorted(samples, key=lambda sample: -sample["keystrokeToPaintMs"])[:count]
