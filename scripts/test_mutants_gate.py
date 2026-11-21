import json
from pathlib import Path
import tempfile
import unittest

from mutants_gate import (
    GateError, classify, exact_name_regex, is_relevant, line_diff, load_config, parse_changes, parse_listing,
    read_results, resolve_replay, scope_args, scope_for, select, selected_or_fail, verdict,
)

CRITICAL = ' in (Server::)?(percent_decode|dispatch)\\b'
CONFIG = {'irrelevant': ['*.md', 'LICENSE'], 'scope': ['src/main.rs'], 'critical': CRITICAL}
DECLARED = (
    'scope = ["src/main.rs"]\n'
    "critical = ' in (Server::)?(percent_decode|dispatch)\\b'\n"
    'irrelevant = ["*.md"]\n'
)


def classes_of(changes):
    return {path: classify(path, status, CONFIG) for path, status in changes.items()}


class SelectionTests(unittest.TestCase):
    def test_test_deletion_and_production_union(self):
        changes = parse_changes('D\0tests/cli.rs\0M\0src/main.rs\0')
        kinds = {classify(p, s, CONFIG) for p, s in changes.items()}
        self.assertEqual(kinds, {'tests', 'production'})
        self.assertEqual(scope_for({p: classify(p, s, CONFIG) for p, s in changes.items()}, CONFIG), {'src/main.rs'})

    def test_classes(self):
        for path, status, expected in [
            ('README.md', 'M', 'irrelevant'), ('Cargo.lock', 'M', 'build configuration'),
            ('.cargo/mutants.toml', 'M', 'gate'), ('src/main.rs', 'D', 'deleted production'),
            ('fixtures/data.json', 'A', 'fixtures'), ('other', 'M', 'unclassified'),
        ]:
            with self.subTest(path=path):
                self.assertEqual(classify(path, status, CONFIG), expected)

    def test_production_only_change_selects_only_its_changed_lines(self):
        critical, diff = select(classes_of({'src/lib.rs': 'M', 'src/main.rs': 'M'}), CONFIG)
        self.assertEqual(critical, set())
        self.assertEqual(diff, ['src/lib.rs', 'src/main.rs'])

    def test_test_only_change_selects_exactly_the_declared_critical_scope(self):
        critical, diff = select(classes_of({'tests/cli.rs': 'D'}), CONFIG)
        self.assertEqual((critical, diff), ({'src/main.rs'}, []))
        args = scope_args(critical, CONFIG)
        self.assertEqual(args, ['-f', 'src/main.rs', '--re', CRITICAL])
        self.assertNotIn('src/', args)

    def test_mixed_change_selects_the_critical_scope_and_the_changed_lines(self):
        critical, diff = select(classes_of({'tests/cli.rs': 'D', 'src/other.rs': 'M'}), CONFIG)
        self.assertEqual((critical, diff), ({'src/main.rs'}, ['src/other.rs']))

    def test_edited_test_code_inside_production_selects_the_critical_scope(self):
        critical, diff = select(classes_of({'src/other.rs': 'M'}), CONFIG, test_hunks=True)
        self.assertEqual((critical, diff), ({'src/main.rs'}, ['src/other.rs']))

    def test_docs_only_change_is_irrelevant(self):
        classes = classes_of({'README.md': 'M', 'LICENSE': 'M'})
        self.assertFalse(is_relevant(classes))
        self.assertEqual(select(classes, CONFIG), (set(), []))
        self.assertFalse(is_relevant(classes_of({'src/old.rs': 'D'})))

    def test_missing_critical_declaration_fails_visibly(self):
        self.assertEqual(load_config(DECLARED)['scope'], ['src/main.rs'])
        for text, message in [
            ('scope = ["src/main.rs"]\nirrelevant = []\n', 'declare critical'),
            ('scope = ["src/main.rs"]\ncritical = " "\nirrelevant = []\n', 'declare critical'),
            ('critical = "x"\nirrelevant = []\n', 'explicit list'),
            ('scope = ["src/"]\ncritical = "x"\nirrelevant = []\n', 'explicit list'),
            ('scope = ["src/*.rs"]\ncritical = "x"\nirrelevant = []\n', 'explicit list'),
            ('scope = []\ncritical = "x"\nirrelevant = []\n', 'explicit list'),
            ('{', 'invalid gate config'),
        ]:
            with self.subTest(text=text), self.assertRaisesRegex(GateError, message):
                load_config(text)

    def test_irrelevant_does_not_select_scope(self):
        self.assertEqual(scope_for({'README.md': classify('README.md', 'M', CONFIG)}, CONFIG), set())

    def test_rename(self):
        self.assertEqual(parse_changes('R100\0src/old.rs\0src/new.rs\0'),
                         {'src/old.rs': 'D', 'src/new.rs': 'A'})

    def test_listing(self):
        self.assertEqual(parse_listing(''), [])
        self.assertEqual(parse_listing('[]'), [])
        self.assertEqual(parse_listing('[{"name": "a"}, {"name": "b"}]'), ['a', 'b'])
        for text in ('{', '[{"file": "a"}]', '[{"name": "a"}, {"name": "a"}]'):
            with self.subTest(text=text), self.assertRaisesRegex(GateError, 'selection error'):
                parse_listing(text)

    def test_empty_relevant_selection(self):
        with self.assertRaisesRegex(GateError, 'empty selection'):
            selected_or_fail(True, [])
        selected_or_fail(False, [])


class ReplayTests(unittest.TestCase):
    def test_replay_selects_its_own_line_and_exact_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'src').mkdir()
            (root / 'src/main.rs').write_text('fn a() {}\n    i += 1;\n')
            self.assertEqual(
                line_diff('src/main.rs:2:7: replace += with -= in a', root),
                '--- a/src/main.rs\n+++ b/src/main.rs\n@@ -2,1 +2,1 @@\n-    i += 1;\n+    i += 1;\n',
            )
            for name in ('src/main.rs:9:1: x', 'nonsense'):
                with self.subTest(name=name), self.assertRaisesRegex(GateError, 'cannot locate'):
                    line_diff(name, root)
        self.assertEqual(exact_name_regex('src/a.rs:2:7: replace += with -= in f'),
                         '^src/a\\.rs:2:7: replace \\+= with -= in f$')

    def test_unconfirmed_timeouts_stay_unresolved(self):
        name = 'src/a.rs:1:1: replace f -> u32 with 0'
        self.assertEqual(resolve_replay(name, [name], 'timeout'), 'confirmed hang')
        self.assertEqual(resolve_replay(name, [name], 'caught'), 'caught')
        self.assertEqual(resolve_replay(name, [name], 'missed'), 'missed')
        for selected, replay in [([], None), ([name, 'src/a.rs:1:9: delete field x'], 'timeout'), ([name], None)]:
            with self.subTest(selected=selected):
                self.assertEqual(resolve_replay(name, selected, replay), 'timeout')
        with self.assertRaisesRegex(GateError, 'unresolved timeout'):
            verdict({name: resolve_replay(name, [], None)})


class ResultsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / 'outcomes.json'

    def write(self, summary='CaughtMutant', count=1):
        kinds = {'CaughtMutant': 'caught', 'MissedMutant': 'missed', 'Timeout': 'timeout', 'Unviable': 'unviable'}
        data = {'outcomes': [{'scenario': 'Baseline', 'summary': 'Success'},
                {'scenario': {'Mutant': {'name': 'mutant'}}, 'summary': summary}],
                'total_mutants': count, 'caught': 0, 'missed': 0, 'timeout': 0, 'unviable': 0}
        data[kinds[summary]] = 1
        self.path.write_text(json.dumps(data))

    def test_missing_and_malformed(self):
        with self.assertRaisesRegex(GateError, 'incomplete'):
            read_results(self.path, ['mutant'])
        self.path.write_text('{')
        with self.assertRaisesRegex(GateError, 'incomplete'):
            read_results(self.path, ['mutant'])

    def test_count_and_identity_mismatch(self):
        self.write(count=2)
        with self.assertRaisesRegex(GateError, 'incomplete'):
            read_results(self.path, ['mutant'])
        self.write()
        with self.assertRaisesRegex(GateError, 'incomplete'):
            read_results(self.path, ['other'])

    def test_missed_timeout_and_confirmed_hang(self):
        for summary, message in [('MissedMutant', 'missed'), ('Timeout', 'unresolved timeout')]:
            self.write(summary)
            with self.assertRaisesRegex(GateError, message):
                verdict(read_results(self.path, ['mutant']))
        verdict({'mutant': 'confirmed hang'})
        self.write()
        verdict(read_results(self.path, ['mutant']))


if __name__ == '__main__':
    unittest.main()
