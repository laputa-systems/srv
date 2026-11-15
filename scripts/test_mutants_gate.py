import json
from pathlib import Path
import tempfile
import unittest

from mutants_gate import (
    GateError, classify, parse_changes, parse_listing, read_results, selected_or_fail, scope_for, verdict,
)

CONFIG = {'irrelevant': ['*.md', 'LICENSE'], 'scope': ['src/main.rs']}


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
