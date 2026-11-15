import fnmatch
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / '.cargo/mutants-gate.toml'
WORK = ROOT / '.work/mutants-gate'
GATE = {'Makefile', '.github/workflows/ci.yml', '.cargo/mutants.toml',
        '.cargo/mutants-gate.toml', 'scripts/mutants_gate.py', 'scripts/test_mutants_gate.py'}


class GateError(Exception):
    pass


def untested(pending):
    shown = ', '.join(pending[:10])
    more = f' and {len(pending) - 10} more' if len(pending) > 10 else ''
    return f'budget exhausted; {len(pending)} untested: {shown}{more}'


def classify(path, status, config):
    if path in GATE:
        return 'gate'
    if any(fnmatch.fnmatchcase(path, pattern) for pattern in config['irrelevant']):
        return 'irrelevant'
    if path.startswith('src/') and path.endswith('.rs'):
        return 'deleted production' if status == 'D' else 'production'
    if path.endswith('.rs') and not path.startswith('src/'):
        return 'tests'
    if path.startswith(('tests/', 'fixtures/')):
        return 'fixtures'
    if path in {'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml', 'build.rs'} or path.startswith('.cargo/') or Path(path).name in {'Cargo.toml', 'Cargo.lock', 'build.rs'}:
        return 'build configuration'
    return 'unclassified'


def scope_for(classes, config):
    triggers = {'tests', 'fixtures', 'build configuration', 'gate', 'unclassified'}
    return set(config['scope']) if any(kind in triggers for kind in classes.values()) else set()


def parse_changes(data):
    fields = data.split('\0')
    changes = {}
    i = 0
    while i < len(fields) and fields[i]:
        status = fields[i]
        i += 1
        if status[0] in 'RC':
            old, new = fields[i:i + 2]
            i += 2
            changes[old] = 'D' if status[0] == 'R' else 'M'
            changes[new] = 'A'
        else:
            changes[fields[i]] = status[0]
            i += 1
    return changes


def read_results(path, selected):
    try:
        data = json.loads(path.read_text())
        outcomes = data['outcomes']
        if not isinstance(outcomes, list):
            raise ValueError('outcomes is not a list')
        results = {}
        summaries = {'CaughtMutant': 'caught', 'MissedMutant': 'missed', 'Timeout': 'timeout', 'Unviable': 'unviable'}
        for outcome in outcomes:
            scenario = outcome['scenario']
            if scenario == 'Baseline':
                if outcome['summary'] != 'Success':
                    raise ValueError('baseline failed')
                continue
            name = scenario['Mutant']['name']
            summary = summaries[outcome['summary']]
            if name in results:
                raise ValueError('duplicate mutant')
            results[name] = summary
        if set(results) != set(selected) or data['total_mutants'] != len(selected):
            raise ValueError('tested count or identities differ from selection')
        for kind in summaries.values():
            if data[kind] != list(results.values()).count(kind):
                raise ValueError('summary count mismatch')
        return results
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise GateError(f'incomplete result: {path}: {error}') from error


def verdict(results):
    if 'missed' in results.values():
        raise GateError('missed mutants: ' + ', '.join(n for n, v in results.items() if v == 'missed'))
    if 'timeout' in results.values():
        raise GateError('unresolved timeout: ' + ', '.join(n for n, v in results.items() if v == 'timeout'))


def parse_listing(text):
    if not text.strip():
        return []
    try:
        names = [entry['name'] for entry in json.loads(text)]
    except (ValueError, KeyError, TypeError) as error:
        raise GateError(f'selection error: {error}') from error
    if len(set(names)) != len(names):
        raise GateError('selection error: duplicate mutant names')
    return names


def selected_or_fail(relevant, names):
    if relevant and not names:
        raise GateError('unexpected empty selection')


def main():
    started = time.monotonic()
    config = tomllib.loads(CONFIG.read_text())
    deadline = started + config['budget_seconds']
    WORK.mkdir(parents=True, exist_ok=True)
    base = sys.argv[1]
    common = ['cargo', 'mutants', '--gitignore', 'true']
    phases = []
    counts = {'caught': 0, 'missed': 0, 'unviable': 0, 'timeout': 0, 'confirmed hang': 0}
    hangs = []
    base_sha = head = version = host = 'unavailable'
    excluded = 'unavailable'
    dirty = ''
    pending = []

    def run(args, *, allowed=(0,), capture=True):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise GateError(untested(pending))
        process = subprocess.Popen(args, cwd=ROOT, text=True, stdout=subprocess.PIPE if capture else None,
                                   stderr=subprocess.PIPE if capture else None, start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=remaining)
        except subprocess.TimeoutExpired as error:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            raise GateError(untested(pending)) from error
        if process.returncode not in allowed:
            raise GateError(f'command failed ({process.returncode}): {args}: {stderr or ""}')
        return stdout if capture else ''

    def listing(args):
        return parse_listing(run(common + ['--list', '--json'] + args))

    try:
        base_sha = run(['git', 'merge-base', base, 'HEAD']).strip()
        head = run(['git', 'rev-parse', 'HEAD']).strip()
        dirty = ' dirty' if run(['git', 'status', '--porcelain', '--untracked-files=normal']).strip() else ''
        host = next(line for line in run(['rustc', '-vV']).splitlines() if line.startswith('host:'))
        changes = parse_changes(run(['git', 'diff', '--merge-base', base, '--name-status', '-M', '-z']))
        for path in run(['git', 'ls-files', '--others', '--exclude-standard', '-z']).split('\0'):
            if path:
                changes[path] = 'A'
        classes = {path: classify(path, status, config) for path, status in changes.items()}
        for path, kind in sorted(classes.items()):
            print(f'{kind}: {path}', flush=True)
        whole = scope_for(classes, config)
        production = [p for p, k in classes.items() if k == 'production']
        diffs = {}
        for path in production:
            diff = run(['git', 'diff', '--merge-base', base, '--no-ext-diff', '--', path])
            if not diff and changes[path] == 'A':
                diff = run(['git', 'diff', '--no-index', '--', '/dev/null', path], allowed=(0, 1))
            diffs[path] = diff
            old = run(['git', 'show', f'{base_sha}:{path}'], allowed=(0, 128))
            current = (ROOT / path).read_text()
            old_line = next((i for i, x in enumerate(old.splitlines(), 1) if x.strip() == '#[cfg(test)]'), 10**9)
            new_line = next((i for i, x in enumerate(current.splitlines(), 1) if x.strip() == '#[cfg(test)]'), 10**9)
            for match in re.finditer(r'^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@', diff, re.M):
                old_end = int(match[1]) + int(match[2] or 1) - 1
                new_end = int(match[3]) + int(match[4] or 1) - 1
                if old_end >= old_line or new_end >= new_line:
                    whole.update(config['scope'])
            if path not in whole:
                file_diff = WORK / 'file.diff'
                file_diff.write_text(diff)
                if not listing(['--in-diff', str(file_diff)]):
                    whole.add(path)
        in_diff = [p for p in production if p not in whole]
        if whole:
            args = sum((['-f', p] for p in sorted(whole)), [])
            phases.append(('whole ' + ', '.join(sorted(whole)), args, listing(args)))
        if in_diff:
            path = WORK / 'pr.diff'
            path.write_text(''.join(diffs[p] for p in in_diff))
            args = ['--in-diff', str(path)]
            phases.append(('diff ' + ', '.join(in_diff), args, listing(args)))
        selected = [n for _, _, names in phases for n in names]
        relevant = any(k not in {'irrelevant', 'deleted production'} for k in classes.values())
        selected_or_fail(relevant, selected)
        if not relevant:
            print('irrelevant change: no mutants selected', flush=True)
        if not relevant:
            return
        version = run(common + ['--version']).strip()
        configured = set(listing([]))
        unconfigured = set(listing(['--no-config']))
        excluded = str(len(unconfigured - configured))
        for index, (label, args, names) in enumerate(phases):
            pending[:] = [n for _, _, ns in phases[index:] for n in ns]
            print(f'phase: {label}; selected {len(names)}', flush=True)
            out = WORK / f'phase-{index}'
            run(common + ['--baseline', 'run', '--timeout', str(config['timeout_seconds']),
                '--build-timeout', str(config['build_timeout_seconds']), '--jobs', str(config['jobs']),
                '--output', str(out)] + args, allowed=(0, 2, 3), capture=False)
            results = read_results(out / 'mutants.out/outcomes.json', names)
            for name, result in results.items():
                counts[result] += 1
                if result != 'timeout':
                    pending.remove(name)
            for number, name in enumerate(n for n, v in results.items() if v == 'timeout'):
                regex = '^' + re.sub(r'([\\.^$|?*+()\[\]{}])', r'\\\1', name) + '$'
                retry_args = args + ['--re', regex]
                if listing(retry_args) != [name]:
                    raise GateError(f'timeout rerun selection error: {name}')
                out = WORK / f'retry-{index}-{number}'
                run(common + ['--baseline', 'skip', '--timeout', str(config['timeout_seconds'] * 3),
                    '--build-timeout', str(config['build_timeout_seconds']), '--jobs', '1',
                    '--output', str(out)] + retry_args, allowed=(0, 2, 3), capture=False)
                rerun = read_results(out / 'mutants.out/outcomes.json', [name])[name]
                counts['timeout'] -= 1
                if rerun == 'timeout':
                    results[name] = 'confirmed hang'
                    hangs.append(name)
                    counts['confirmed hang'] += 1
                else:
                    results[name] = rerun
                    counts[rerun] += 1
                pending.remove(name)
            verdict(results)
    finally:
        print('\nEvidence:', flush=True)
        print(f'base: {base_sha}; head: {head}{dirty}; {version}', flush=True)
        print(f'config sha256: {hashlib.sha256(CONFIG.read_bytes()).hexdigest()}; excluded mutants: {excluded}', flush=True)
        print(f'{host}; features: default', flush=True)
        for label, _, names in phases:
            print(f'phase: {label}; selected: {len(names)}', flush=True)
        print('counts: ' + ', '.join(f'{k}={v}' for k, v in counts.items()), flush=True)
        print('confirmed hangs: ' + (', '.join(hangs) if hangs else 'none'), flush=True)
        print(f'wall time: {time.monotonic() - started:.1f}s', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (GateError, OSError, ValueError, StopIteration) as error:
        print(f'gate failure: {error}', file=sys.stderr)
        sys.exit(1)
