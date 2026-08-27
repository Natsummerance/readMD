# -*- coding: utf-8 -*-
"""Run five privacy-safe ReadMD startup probes and print their JSON summaries.

Usage: ``python readmd_startup_probe.py``.  The app closes itself after each
page-ready signal; no document is opened or recorded.
"""

import argparse
import math
import json
import os
import subprocess
import sys
import tempfile


def percentile(values, percent):
    """Nearest-rank percentile for a non-empty list of numbers."""
    if not values:
        raise ValueError('percentile requires at least one value')
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(percent * len(ordered)) - 1))
    return ordered[index]


def evaluate_startup_reports(reports, max_page_loaded_ms, max_window_overhead_ms):
    """Return a privacy-safe numeric release gate for startup reports."""
    samples = []
    failures = []
    for report in reports:
        run = report.get('run', len(samples) + 1)
        if report.get('returncode') != 0:
            failures.append(f'run {run}: process exited with {report.get("returncode")}')
            continue
        if report.get('timed_out'):
            failures.append(f'run {run}: startup timed out')
            continue
        milestones = report.get('milestones_ms') or {}
        page_loaded = milestones.get('page_loaded')
        server_up = milestones.get('server_up')
        window_created = milestones.get('window_created')
        if not isinstance(page_loaded, (int, float)) or page_loaded < 0:
            failures.append(f'run {run}: page_loaded missing')
            continue
        if not isinstance(server_up, (int, float)) or not isinstance(window_created, (int, float)):
            failures.append(f'run {run}: server/window milestone missing')
            continue
        overhead = window_created - server_up
        if overhead < 0:
            failures.append(f'run {run}: window milestone precedes server milestone')
            continue
        samples.append({
            'run': run,
            'page_loaded_ms': page_loaded,
            'server_to_window_ms': overhead,
        })

    page_p95 = percentile([item['page_loaded_ms'] for item in samples], 0.95) if samples else None
    window_overhead_p95 = percentile(
        [item['server_to_window_ms'] for item in samples], 0.95,
    ) if samples else None
    if page_p95 is not None and page_p95 > max_page_loaded_ms:
        failures.append(f'page_loaded P95 {page_p95:.1f}ms exceeds {max_page_loaded_ms}ms')
    if window_overhead_p95 is not None and window_overhead_p95 > max_window_overhead_ms:
        failures.append(
            f'server-to-window P95 {window_overhead_p95:.1f}ms exceeds {max_window_overhead_ms}ms'
        )

    return {
        'ok': not failures,
        'runs': len(reports),
        'valid_runs': len(samples),
        'budgets_ms': {
            'page_loaded_p95': max_page_loaded_ms,
            'server_to_window_p95': max_window_overhead_ms,
        },
        'measured_ms': {
            'page_loaded_p95': page_p95,
            'server_to_window_p95': window_overhead_p95,
        },
        'samples': samples,
        'failures': failures,
    }


def main():
    parser = argparse.ArgumentParser(description='repeat ReadMD startup probes')
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--timeout', type=float, default=20)
    parser.add_argument('--executable', help='packaged ReadMD executable (default: source entrypoint)')
    parser.add_argument('--output', help='write the aggregate gate report as JSON')
    parser.add_argument('--max-page-loaded-ms', type=float, default=2000)
    parser.add_argument('--max-window-overhead-ms', type=float, default=120)
    args = parser.parse_args()
    if args.runs <= 0 or args.timeout <= 0:
        parser.error('--runs and --timeout must be positive')
    if args.max_page_loaded_ms <= 0 or args.max_window_overhead_ms <= 0:
        parser.error('startup budgets must be positive')
    app = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'readmd.py')

    reports = []
    with tempfile.TemporaryDirectory(prefix='readmd-startup-probe-') as directory:
        for index in range(args.runs):
            output = os.path.join(directory, '%02d.json' % (index + 1))
            command_tail = [
                '--startup-probe', '--startup-probe-json', output,
                '--startup-probe-timeout', str(args.timeout),
            ]
            result = subprocess.run(
                ([sys.executable, app] if not args.executable else [args.executable]) + command_tail,
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
            report = {'run': index + 1, 'returncode': result.returncode}
            try:
                with open(output, encoding='utf-8') as handle:
                    report.update(json.load(handle))
            except OSError:
                report['error'] = report.get('error', 'probe did not produce JSON')
            reports.append(report)
    gate = evaluate_startup_reports(
        reports, args.max_page_loaded_ms, args.max_window_overhead_ms,
    )
    encoded = json.dumps(gate, ensure_ascii=False, indent=2)
    print(encoded)
    if args.output:
        os.makedirs(os.path.dirname(os.path.abspath(args.output)), exist_ok=True)
        temporary = args.output + '.tmp'
        with open(temporary, 'w', encoding='utf-8') as handle:
            handle.write(encoded)
        os.replace(temporary, args.output)
    return 0 if gate['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
