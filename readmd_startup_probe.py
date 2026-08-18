# -*- coding: utf-8 -*-
"""Run five privacy-safe ReadMD startup probes and print their JSON summaries.

Usage: ``python readmd_startup_probe.py``.  The app closes itself after each
page-ready signal; no document is opened or recorded.
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description='repeat ReadMD startup probes')
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--timeout', type=float, default=20)
    args = parser.parse_args()
    if args.runs <= 0 or args.timeout <= 0:
        parser.error('--runs and --timeout must be positive')
    app = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'readmd.py')
    reports = []
    with tempfile.TemporaryDirectory(prefix='readmd-startup-probe-') as directory:
        for index in range(args.runs):
            output = os.path.join(directory, '%02d.json' % (index + 1))
            result = subprocess.run(
                [sys.executable, app, '--startup-probe', '--startup-probe-json', output,
                 '--startup-probe-timeout', str(args.timeout)],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
            report = {'run': index + 1, 'returncode': result.returncode}
            try:
                with open(output, encoding='utf-8') as handle:
                    report.update(json.load(handle))
            except OSError:
                report['error'] = 'probe did not produce JSON'
            reports.append(report)
    print(json.dumps(reports, ensure_ascii=False, indent=2))
    return 0 if all(item['returncode'] == 0 for item in reports) else 1


if __name__ == '__main__':
    sys.exit(main())
