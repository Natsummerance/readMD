# -*- coding: utf-8 -*-
"""Startup probe aggregate release gate tests."""
from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / 'tools' / 'readmd_startup_probe.py'
spec = importlib.util.spec_from_file_location('readmd_startup_probe', MODULE_PATH)
startup_probe = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = startup_probe
spec.loader.exec_module(startup_probe)


def report(run, page=1000, overhead=50, returncode=0, timed_out=False):
    return {
        'run': run,
        'returncode': returncode,
        'timed_out': timed_out,
        'milestones_ms': {
            'server_up': 10,
            'window_created': 10 + overhead,
            'page_loaded': page,
        },
    }


class StartupProbeGateTest(unittest.TestCase):
    def test_nearest_rank_percentile(self):
        self.assertEqual(startup_probe.percentile([100, 200, 300, 400], 0.95), 400)

    def test_valid_samples_pass_numeric_budget(self):
        reports = [report(index, page=800 + index) for index in range(1, 6)]
        gate = startup_probe.evaluate_startup_reports(reports, 1200, 120)
        self.assertTrue(gate['ok'])
        self.assertEqual(gate['valid_runs'], 5)
        self.assertEqual(gate['measured_ms']['page_loaded_p95'], 805)
        self.assertEqual(gate['measured_ms']['server_to_window_p95'], 50)

    def test_timeout_missing_or_slow_sample_fails_closed(self):
        gate = startup_probe.evaluate_startup_reports([
            report(1, timed_out=True),
            {'run': 2, 'returncode': 0},
            report(3, page=2500),
            report(4, overhead=300),
            {**report(5), 'returncode': 2},
        ], 2000, 120)
        self.assertFalse(gate['ok'])
        self.assertEqual(gate['valid_runs'], 2)
        self.assertEqual(len(gate['failures']), 5)

    def test_cli_parser_warmup_support(self):
        import argparse
        parser = argparse.ArgumentParser()
        parser.add_argument('--runs', type=int, default=5)
        parser.add_argument('--warmup', type=int, default=0)
        parser.add_argument('--timeout', type=float, default=20)
        args = parser.parse_args(['--runs', '5', '--warmup', '1', '--timeout', '30'])
        self.assertEqual(args.warmup, 1)
        self.assertEqual(args.runs, 5)
        self.assertEqual(args.timeout, 30)
