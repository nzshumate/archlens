#!/usr/bin/env python3
"""Portable wall-clock benchmark; reports measurements without performance claims."""
import argparse
import json
import os
import pathlib
import statistics
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('target', nargs='?', default='.')
parser.add_argument('--runs', type=int, default=int(os.environ.get('RUNS', '5')))
parser.add_argument('--binary', default='target/release/oxarch')
args = parser.parse_args()
if args.runs < 1:
    parser.error('--runs must be positive')
binary = str(pathlib.Path(args.binary).resolve())
command = [binary, 'analyze', args.target, '--json']
# Warmup populates the cache and verifies the command before measurement.
subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
results = {}
for mode, extra in [('uncached', ['--no-cache']), ('warm_cache', [])]:
    timings = []
    for _ in range(args.runs):
        start = time.perf_counter()
        subprocess.run(command + extra, check=True, stdout=subprocess.DEVNULL)
        timings.append(time.perf_counter() - start)
    results[mode] = {'seconds': timings, 'median_seconds': statistics.median(timings)}
print(json.dumps({'target': str(pathlib.Path(args.target).resolve()), 'runs': args.runs, 'measurements': results}, indent=2))
