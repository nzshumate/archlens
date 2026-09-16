#!/usr/bin/env python3
"""Exercise the real loopback server and refresh/error routes without a browser."""
import argparse
import json
import pathlib
import select
import subprocess
import tempfile
import urllib.error
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--binary', default='target/debug/oxarch')
args = parser.parse_args()
binary = str(pathlib.Path(args.binary).resolve())
with tempfile.TemporaryDirectory(prefix='oxarch-http-') as directory:
    root = pathlib.Path(directory)
    (root / 'main.ts').write_text('export {};')
    process = subprocess.Popen([binary, 'dev', directory, '--port', '0'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        ready, _, _ = select.select([process.stdout], [], [], 15)
        assert ready, 'Explorer did not start within 15 seconds'
        line = process.stdout.readline().strip()
        assert line.startswith('Oxarch explorer: http://127.0.0.1:'), line
        url = line.split(': ', 1)[1]

        def request(path, method='GET'):
            try:
                response = urllib.request.urlopen(urllib.request.Request(url + path, method=method), timeout=10)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                return response.status, response.headers, response.read().decode()

        status, headers, body = request('/')
        assert status == 200 and 'Refresh analysis' in body
        assert 'frame-ancestors' in headers['Content-Security-Policy']
        assert request('/explorer.js')[0] == 200
        assert request('/missing')[0] == 404
        assert request('/api/report', 'POST')[0] == 405
        first = json.loads(request('/api/report')[2])
        assert first['schema_version'] == 1
        assert first['analysis']['source_files'] == 1 and first['diff'] is None
        assert first['analysis']['diagnostics'] == []
        assert first['guidance']['boundary_rule_count'] == 0
        assert 'not checked' in first['guidance']['boundaries']
        assert first['project'] == root.name
        (root / 'main.ts').write_text("import './new';")
        (root / 'new.ts').write_text('export {};')
        second = json.loads(request('/api/report')[2])
        assert second['analysis']['dependencies'] == 1
        (root / 'oxarch.json').write_text(json.dumps({'entryPoints': ['main.ts']}))
        (root / 'unused.ts').write_text('export {};')
        (root / 'broken.ts').write_text('export const = ;')
        diagnostic = json.loads(request('/api/report')[2])
        assert diagnostic['metrics']['reachability_mode'] == 'explicit'
        assert diagnostic['metrics']['dead_candidates'] == ['broken.ts', 'unused.ts']
        assert diagnostic['analysis']['diagnostics'][0]['code'] == 'parse_error'
        assert diagnostic['guidance']['findings'][0]['kind'] == 'parse_error'
        assert diagnostic['guidance']['findings'][0]['action']
        (root / '.oxarchignore').write_text('broken.ts\n')
        recovered = json.loads(request('/api/report')[2])
        assert recovered['analysis']['diagnostics'] == []
        assert recovered['metrics']['dead_candidates'] == ['unused.ts']
        (root / 'oxarch.json').write_text('invalid rules')
        assert request('/api/report')[0] == 500
        (root / 'oxarch.json').write_text('{}')
        assert request('/api/report')[0] == 200
        print('Explorer HTTP checks passed: assets, routing, diagnostics, entry points, ignores, refresh, error and recovery.')
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
