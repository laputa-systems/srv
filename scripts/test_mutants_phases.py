"""Run the real gate against a cheap Git fixture with a stub cargo-mutants and check its phases."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

SCRIPTS = Path(__file__).resolve().parent
REPO = SCRIPTS.parent

MAIN = """\
fn dispatch(x: i32) -> i32 {
    x + 1
}

fn helper(x: i32) -> i32 {
    x * 2
}

fn main() {
    println!("{}", dispatch(helper(1)));
}
"""

CRITICAL_MUTANT = 'src/main.rs:2:7: replace + with - in dispatch'
CHANGED_MUTANT = 'src/main.rs:6:7: replace * with + in helper'

# Lists CRITICAL_MUTANT for a -f/--re scope and CHANGED_MUTANT for an --in-diff
# selection that touches helper, and records every invocation.
STUB = textwrap.dedent('''\
    import json, os, sys
    from pathlib import Path
    args = sys.argv[1:]
    with open(os.environ['STUB_LOG'], 'a') as log:
        log.write(json.dumps(args) + '\\n')
    if args[0] == '-vV':
        print('host: fixture')
        raise SystemExit
    if '--version' in args:
        print('cargo-mutants 27.1.0')
        raise SystemExit
    names = []
    if '--in-diff' in args:
        diff = Path(args[args.index('--in-diff') + 1]).read_text()
        if 'x * 3' in diff:
            names.append({changed!r})
    elif '-f' in args:
        names.append({critical!r})
    else:
        names += [{critical!r}, {changed!r}]
    if '--list' in args:
        print(json.dumps([{{'name': n}} for n in names]))
        raise SystemExit
    out = Path(args[args.index('--output') + 1]) / 'mutants.out'
    out.mkdir(parents=True)
    outcomes = [{{'scenario': 'Baseline', 'summary': 'Success'}}]
    outcomes += [{{'scenario': {{'Mutant': {{'name': n}}}}, 'summary': 'CaughtMutant'}} for n in names]
    (out / 'outcomes.json').write_text(json.dumps({{
        'outcomes': outcomes, 'total_mutants': len(names),
        'caught': len(names), 'missed': 0, 'timeout': 0, 'unviable': 0,
    }}))
''').format(critical=CRITICAL_MUTANT, changed=CHANGED_MUTANT)


class PhaseSelectionTests(unittest.TestCase):
    def setUp(self):
        scratch = REPO / '.work' / 'tmp'
        scratch.mkdir(parents=True, exist_ok=True)
        self.directory = Path(tempfile.mkdtemp(prefix='mutants-phases-', dir=scratch))
        self.addCleanup(shutil.rmtree, self.directory, ignore_errors=True)
        self.root = self.directory / 'repo'
        (self.root / 'scripts').mkdir(parents=True)
        (self.root / 'src').mkdir()
        (self.root / 'tests').mkdir()
        (self.root / '.cargo').mkdir()
        shutil.copyfile(SCRIPTS / 'mutants_gate.py', self.root / 'scripts/mutants_gate.py')
        shutil.copyfile(REPO / '.cargo/mutants-gate.toml', self.root / '.cargo/mutants-gate.toml')
        (self.root / '.gitignore').write_text('/.work\n')
        (self.root / 'src/main.rs').write_text(MAIN)
        (self.root / 'tests/cli.rs').write_text('#[test]\nfn runs() {}\n')
        bin_dir = self.directory / 'bin'
        bin_dir.mkdir()
        (bin_dir / 'stub.py').write_text(STUB)
        for tool in ('cargo', 'rustc'):
            launcher = bin_dir / tool
            launcher.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{bin_dir / "stub.py"}" "$@"\n')
            launcher.chmod(0o755)
        self.log = self.directory / 'stub.log'
        self.env = {
            **os.environ, 'PATH': f'{bin_dir}{os.pathsep}{os.environ["PATH"]}', 'STUB_LOG': str(self.log),
            'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_CONFIG_NOSYSTEM': '1',
            'GIT_AUTHOR_NAME': 'fixture', 'GIT_AUTHOR_EMAIL': 'fixture@invalid',
            'GIT_COMMITTER_NAME': 'fixture', 'GIT_COMMITTER_EMAIL': 'fixture@invalid',
        }
        self.git('init', '-q', '-b', 'main')
        self.git('add', '-A')
        self.git('commit', '-q', '-m', 'base')

    def git(self, *args):
        subprocess.run(['git', *args], cwd=self.root, env=self.env, check=True, capture_output=True)

    def gate(self, *, production, tests):
        if production:
            (self.root / 'src/main.rs').write_text(MAIN.replace('x * 2', 'x * 3'))
        if tests:
            (self.root / 'tests/cli.rs').write_text('#[test]\nfn runs() { assert!(true); }\n')
        self.git('commit', '-q', '-am', 'change')
        result = subprocess.run(
            [sys.executable, '-B', 'scripts/mutants_gate.py', 'HEAD~1'],
            cwd=self.root, env=self.env, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        phases = [line for line in result.stdout.splitlines() if line.startswith('phase: ') and '; selected ' in line]
        runs = [args for args in map(json.loads, self.log.read_text().splitlines()) if '--output' in args]
        return phases, runs

    def test_mixed_change_runs_the_changed_lines_and_the_declared_critical_scope(self):
        phases, runs = self.gate(production=True, tests=True)
        self.assertEqual(phases, ['phase: critical src/main.rs; selected 1', 'phase: diff src/main.rs; selected 1'])
        self.assertEqual(len(runs), 2)
        critical, diff = runs
        self.assertEqual(critical[critical.index('-f') + 1], 'src/main.rs')
        self.assertIn('--re', critical)
        self.assertIn('--in-diff', diff)
        self.assertNotIn('-f', diff)

    def test_production_only_change_runs_only_the_changed_lines(self):
        phases, runs = self.gate(production=True, tests=False)
        self.assertEqual(phases, ['phase: diff src/main.rs; selected 1'])
        self.assertEqual(len(runs), 1)
        self.assertIn('--in-diff', runs[0])

    def test_test_only_change_runs_only_the_declared_critical_scope(self):
        phases, runs = self.gate(production=False, tests=True)
        self.assertEqual(phases, ['phase: critical src/main.rs; selected 1'])
        self.assertEqual(len(runs), 1)
        self.assertEqual(runs[0][runs[0].index('-f') + 1], 'src/main.rs')


if __name__ == '__main__':
    unittest.main()
