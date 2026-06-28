#!/usr/bin/env python3
"""Verify round-trip for a single microtest file across all parsers."""
import subprocess, sys, tempfile, os, json

def roundtrip(qmdc_path, cli):
    """Parse then rebuild, return rebuilt text."""
    r = subprocess.run([cli, 'parse', '-i', qmdc_path],
                       capture_output=True, text=True, timeout=30)
    if r.returncode != 0:
        return f"PARSE ERROR: {r.stderr[:200]}"
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as tmp:
        tmp.write(r.stdout)
        tmp_path = tmp.name
    try:
        r2 = subprocess.run([cli, 'rebuild', '-i', tmp_path],
                            capture_output=True, text=True, timeout=30)
    finally:
        os.unlink(tmp_path)
    if r2.returncode != 0:
        return f"REBUILD ERROR: {r2.stderr[:200]}"
    return r2.stdout

def roundtrip_ts_batch(qmdc_paths):
    """Run TS parse+rebuild for multiple files in a single npx tsx call."""
    script = '''
import { readFileSync } from 'fs';
import { parse, rebuild } from './src/parser.js';
const files = JSON.parse(readFileSync(process.argv[2], 'utf-8'));
const results = {};
for (const f of files) {
    try {
        const original = readFileSync(f, 'utf-8');
        const parsed = parse(original);
        const rebuilt = rebuild(parsed);
        results[f] = { rebuilt, err: null };
    } catch (e) {
        results[f] = { rebuilt: null, err: e.message.slice(0, 200) };
    }
}
process.stdout.write(JSON.stringify(results));
'''
    batch_path = os.path.join('qmdc-ts', '_verify_batch.ts')
    file_list_path = None
    try:
        with open(batch_path, 'w') as bf:
            bf.write(script)
        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as fl:
            json.dump(qmdc_paths, fl)
            file_list_path = fl.name
        r = subprocess.run(
            ['npx', 'tsx', batch_path, file_list_path],
            capture_output=True, text=True, timeout=60, cwd='.'
        )
        if r.returncode != 0:
            return {p: f"TS ERROR: {r.stderr[:200]}" for p in qmdc_paths}
        raw = json.loads(r.stdout)
        result = {}
        for p in qmdc_paths:
            entry = raw.get(p, {})
            if entry.get('err'):
                result[p] = f"TS ERROR: {entry['err']}"
            else:
                result[p] = entry.get('rebuilt', '')
        return result
    finally:
        if os.path.exists(batch_path):
            os.unlink(batch_path)
        if file_list_path and os.path.exists(file_list_path):
            os.unlink(file_list_path)

if __name__ == '__main__':
    qmdc_paths = sys.argv[1:]
    if not qmdc_paths:
        print("Usage: verify_roundtrip.py <file1.qmd.md> [file2.qmd.md ...]")
        sys.exit(1)

    # Get TS results in batch
    ts_results = roundtrip_ts_batch(qmdc_paths)

    any_mismatch = False

    for qmdc_path in qmdc_paths:
        original = open(qmdc_path).read()
        print(f"=== ORIGINAL: {qmdc_path} ({len(original)} chars) ===")
        print(original)
        for name, cli in [('Rust', './bin/qmdc-rs'), ('Python', './bin/qmdc-py')]:
            rebuilt = roundtrip(qmdc_path, cli)
            match = "MATCH" if rebuilt == original else "MISMATCH"
            if match == "MISMATCH":
                any_mismatch = True
            print(f"=== {name} rebuild ({len(rebuilt)} chars) [{match}] ===")
            if match == "MISMATCH":
                print(rebuilt)
            else:
                print("  (identical)")
        # TS
        ts_rebuilt = ts_results.get(qmdc_path, 'TS NOT RUN')
        if isinstance(ts_rebuilt, str) and ts_rebuilt.startswith('TS ERROR'):
            any_mismatch = True
            print(f"=== TS rebuild [ERROR] ===")
            print(ts_rebuilt)
        else:
            match = "MATCH" if ts_rebuilt == original else "MISMATCH"
            if match == "MISMATCH":
                any_mismatch = True
            print(f"=== TS rebuild ({len(ts_rebuilt)} chars) [{match}] ===")
            if match == "MISMATCH":
                print(ts_rebuilt)
            else:
                print("  (identical)")
        print()

    sys.exit(1 if any_mismatch else 0)
