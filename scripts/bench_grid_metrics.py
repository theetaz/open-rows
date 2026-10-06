# SPDX-License-Identifier: Apache-2.0
"""Validate canvas sweeps; gate best repeats and tracked work in every repeat."""
import math
import statistics
from bench_ipc_metrics import timer_tolerance, valid_timing
from bench_metrics import BenchmarkError, summary

SCENARIOS = ("smooth", "flings", "horizontal", "jump")
DURATIONS = dict(smooth=12_500, flings=14_500, horizontal=7_500, jump=2_000)
MAX_DURATIONS = dict(DURATIONS, jump=4_000)


def frame_summary(values):
    return dict(summary(values), p99=sorted(values)[math.ceil(len(values) * .99) - 1],
                over_20=sum(value > 20 for value in values),
                refresh_interval_ms=statistics.median(values),
                over_32=sum(value > 32 for value in values))


def validate_run(sample, tolerance):
    if not isinstance(sample, dict):
        raise BenchmarkError("Invalid grid scenario")
    option, scenario = sample.get("option"), sample.get("scenario")
    if option != "own" or scenario not in SCENARIOS:
        raise BenchmarkError("Unknown grid option/scenario")
    validate_samples(sample, tolerance)
    validate_workload(sample)
    frames, paint = sample['frameMs'], sample['paintMs']
    return option, scenario, dict(frame_summary(frames), mean_paint_ms=statistics.fmean(paint),
                                 frames=len(frames), own_cost_over_32=sum(item['frameMs'] > 32 and tracked_work(item) >= 16
                                                                      for item in sample['frameBreakdown']),
                                 tiles=sample['tilesRequested'], hits=sample['cacheHits'],
                                 blank_frames=sample['blankCellFrames'], paint_method=sample['paintMethod'])


def validate_samples(sample, tolerance):
    frames, paint = sample.get("frameMs"), sample.get("paintMs")
    if not isinstance(frames, list) or not frames or not isinstance(paint, list) or len(paint) != len(frames):
        raise BenchmarkError("Missing or mismatched grid frame/paint samples")
    if not all(valid_timing(value) and value > 0 for value in frames) or not all(valid_timing(value) for value in paint):
        raise BenchmarkError("Invalid grid timing")
    validate_breakdown(sample, frames, paint)
    scenario = sample["scenario"]
    duration = sample.get("durationMs")
    if not valid_timing(duration) or duration < DURATIONS[scenario] or duration > MAX_DURATIONS[scenario] + 100:
        raise BenchmarkError("Incomplete or over-budget grid duration")
    if abs(sum(frames) - duration) > tolerance + 1e-6:
        raise BenchmarkError("Frame samples omit part of the scenario")
    for key in ("tilesRequested", "cacheHits", "blankCellFrames"):
        if type(sample.get(key)) is not int or sample[key] < 0:
            raise BenchmarkError("Invalid grid counters")
    if sample['blankCellFrames'] > len(frames) or sample['tilesRequested'] == 0:
        raise BenchmarkError("Invalid blank-frame count or missing tile path")


def validate_workload(sample):
    scenario = sample["scenario"]
    expected_row = 999_977 if scenario == "smooth" else 500_000 if scenario == "horizontal" else 501_324 if scenario == "flings" else 999_000
    if sample.get('startRow') != 0 or sample.get('endRow') != expected_row:
        raise BenchmarkError("Grid scroll endpoint was not reached")
    if sample.get('cols') != (100 if scenario == "horizontal" else 20):
        raise BenchmarkError("Wrong grid column workload")
    if scenario == "horizontal" and sample.get('endColumn') != 99:
        raise BenchmarkError("Horizontal sweep did not reach column 100")
    if not valid_timing(sample.get('maxScrollErrorPx')) or sample['maxScrollErrorPx'] > 1000:
        raise BenchmarkError('Native grid did not follow commanded scroll offsets')
    if not isinstance(sample.get('paintMethod'), str) or not sample['paintMethod']:
        raise BenchmarkError("Missing paint method")


def grid_metrics(report, pid, repeat=1):
    if not isinstance(report, dict) or report.get('error') is not None:
        raise BenchmarkError("Grid spike failed or returned an invalid report")
    expected = dict(schema_version=1, pid=pid, seed=42, rows=1_000_000, cols=20, mode='grid')
    if any(report.get(key) != value for key, value in expected.items()):
        raise BenchmarkError("Unexpected grid workload/schema/PID")
    tolerance = timer_tolerance(report)
    samples = report.get('grid')
    if not isinstance(samples, list) or len(samples) != 4:
        raise BenchmarkError("Expected all four canvas scenarios")
    output = {}
    for sample in samples:
        option, scenario, metrics = validate_run(sample, tolerance)
        key = f'{option}/{scenario}/repeat-{repeat}'
        if key in output:
            raise BenchmarkError("Repeated grid scenario")
        output[key] = metrics
    if set(output) != {f'own/{scenario}/repeat-{repeat}' for scenario in SCENARIOS}:
        raise BenchmarkError("Missing grid scenario")
    return output


def tracked_work(frame):
    """Conservative inclusive main-thread timing sum; worker CPU is diagnostic only."""
    return sum(frame[field] for field in ('scrollHandlerMs', 'reactCommitMs', 'canvasResizeMs',
               'prepareMs', 'paintMs', 'tileApplyMs', 'tileConvertMs', 'driverMs'))


def grid_budgets(metrics, repeats):
    if type(repeats) is not int or repeats < 1:
        raise BenchmarkError('Invalid grid repeat count')
    expected = {f'own/{scenario}/repeat-{repeat}' for scenario in SCENARIOS for repeat in range(1, repeats + 1)}
    if set(metrics) != expected:
        raise BenchmarkError('Missing or unexpected grid repeats')
    budgets = []
    for scenario in SCENARIOS:
        candidates = [(key, value) for key, value in metrics.items() if key.startswith(f'own/{scenario}/')]
        key, best = min(candidates, key=lambda pair: (pair[1]['over_32'], pair[1]['max']))
        budgets.append(dict(metric=f'{scenario} best repeat frames >32ms ({key})', actual=best['over_32'],
                            limit=0, unit='frames', passed=best['over_32'] == 0))
    for key, value in metrics.items():
        cost = value['own_cost_over_32']
        budgets.append(dict(metric=f'{key} frames >32ms with tracked work >=16ms', actual=cost,
                            limit=0, unit='frames', passed=cost == 0))
    return budgets


def validate_breakdown(sample, frames, paint):
    breakdown = sample.get('frameBreakdown')
    if not isinstance(breakdown, list) or len(breakdown) != len(frames):
        raise BenchmarkError('Missing per-frame breakdown')
    timings = ('scrollHandlerMs', 'reactCommitMs', 'canvasResizeMs', 'prepareMs',
               'paintMs', 'tileApplyMs', 'tileConvertMs', 'workerConvertMs', 'driverMs')
    counts = ('canvasResizeEvents', 'tileAllocations', 'arrayAllocations', 'cellAllocations')
    for index, item in enumerate(breakdown):
        if not isinstance(item, dict) or item.get('index') != index or item.get('frameMs') != frames[index]:
            raise BenchmarkError('Breakdown does not match frame samples')
        if item.get('paintMs') != paint[index] or not isinstance(item.get('step'), str) or not item['step']:
            raise BenchmarkError('Missing scenario step or mismatched paint')
        if not all(valid_timing(item.get(key)) for key in timings):
            raise BenchmarkError('Invalid breakdown timing')
        if not all(type(item.get(key)) is int and item[key] >= 0 for key in counts):
            raise BenchmarkError('Invalid allocation/resize counter')
    expected = sorted(breakdown, key=lambda item: -item['frameMs'])[:10]
    if sample.get('longestFrames') != expected:
        raise BenchmarkError('Top ten frames do not match full breakdown')


def longest_frames(report):
    """Top ten canvas intervals across scenarios in a repeat, with attribution."""
    frames = [dict(frame, scenario=sample['scenario'], option='own')
              for sample in (report.get('grid') or []) if isinstance(sample, dict) and sample.get('option') == 'own'
              for frame in sample.get('longestFrames', [])]
    return sorted(frames, key=lambda item: -item['frameMs'])[:10]
