"""Fictional fixtures only. No network, real identifiers, or CCP archive needed."""
import hashlib
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from convert import convert, write_output


class ConverterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.systems = [{'_key': 1, 'name': {'en': 'Fiction Alpha'}, 'stargateIDs': [11]},
                        {'_key': 2, 'stargateIDs': [12]}, {'_key': 3}]
        self.gates = [{'_key': 11, 'solarSystemID': 1, 'destination': {'solarSystemID': 2, 'stargateID': 12}},
                      {'_key': 12, 'solarSystemID': 2, 'destination': {'solarSystemID': 1, 'stargateID': 11}}]

    def archive(self, extra=None, raw=None):
        p = self.root / 'fiction.zip'
        with zipfile.ZipFile(p, 'w', zipfile.ZIP_DEFLATED) as z:
            z.writestr('_sde.jsonl', json.dumps({'_key': 'sde', 'buildNumber': 42, 'releaseDate': '2026-01-01T00:00:00Z'}))
            z.writestr('mapSolarSystems.jsonl', raw if raw is not None else ''.join(json.dumps(x)+'\n' for x in self.systems))
            z.writestr('mapStargates.jsonl', ''.join(json.dumps(x)+'\n' for x in self.gates))
            if extra:
                z.writestr(*extra)
        return p

    def run_convert(self, **kw):
        p = self.archive(**kw)
        return convert(p, 42, hashlib.sha256(p.read_bytes()).hexdigest())

    def test_determinism_and_isolated(self):
        a, meta = self.run_convert()
        self.systems.reverse()
        self.gates.reverse()
        b, _ = self.run_convert()
        self.assertEqual(a, b)
        self.assertEqual(json.loads(meta)['isolatedSystems'], 1)
        self.assertEqual(json.loads(meta)['gates'], 2)
        self.assertEqual(json.loads(a)['systems'][1], [2, None])

    def test_nonoverwrite(self):
        a, b = self.run_convert()
        out = self.root / 'output'
        write_output(out, a, b)
        with self.assertRaises(FileExistsError):
            write_output(out, 'bad', 'bad')
        self.assertEqual((out / 'snapshot.json').read_text(), a)

    def test_unsafe_members(self):
        for name in ('../escape', '/absolute', 'C:drive', 'a\\b', 'a/../b'):
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.run_convert(extra=(name, 'x'))
        info = zipfile.ZipInfo('link')
        info.create_system = 3
        info.external_attr = (stat.S_IFLNK | 0o777) << 16
        with self.assertRaises(ValueError):
            self.run_convert(extra=(info, 'target'))

    def test_inflated(self):
        with self.assertRaises(ValueError):
            self.run_convert(extra=('bomb', '0' * 2_000_000))

    def test_bad_json(self):
        for raw in ('{}\n{}\n', '{"_key":1,"_key":2}\n', '[]\n', '{bad}\n', '\n', 'x' * 65537):
            with self.subTest(raw=raw[:25]), self.assertRaises(ValueError):
                self.run_convert(raw=raw)

    def test_bad_systems(self):
        for key in (0, -1, True, 1.5, 9007199254740992):
            self.systems[0]['_key'] = key
            with self.assertRaises(ValueError):
                self.run_convert()
        self.systems[0]['_key'] = 1
        self.systems.append(self.systems[0])
        with self.assertRaises(ValueError):
            self.run_convert()

    def test_gate_contradictions(self):
        for field, value in [('stargateID', 999), ('solarSystemID', 999), ('stargateID', 11), ('solarSystemID', 1)]:
            self.gates[0]['destination'] = {'solarSystemID': 2, 'stargateID': 12}
            self.gates[0]['destination'][field] = value
            with self.assertRaises(ValueError):
                self.run_convert()

    def test_missing_duplicate_unknown(self):
        original = list(self.gates)
        for gates in (original[:1], original + original[:1], [dict(original[0], solarSystemID=999)]):
            self.gates = gates
            with self.assertRaises(ValueError):
                self.run_convert()

    def test_duplicate_connections_and_gate_declarations(self):
        self.systems[0]['stargateIDs'].append(21)
        self.systems[1]['stargateIDs'].append(22)
        self.gates += [
            {'_key': 21, 'solarSystemID': 1, 'destination': {'solarSystemID': 2, 'stargateID': 22}},
            {'_key': 22, 'solarSystemID': 2, 'destination': {'solarSystemID': 1, 'stargateID': 21}}]
        with self.assertRaises(ValueError):
            self.run_convert()
        self.gates = self.gates[:2]
        self.systems[0]['stargateIDs'] = [11, 11]
        with self.assertRaises(ValueError):
            self.run_convert()

    def test_missing_table_duplicate_member_and_time_limit(self):
        p = self.root / 'empty.zip'
        with zipfile.ZipFile(p, 'w') as z:
            z.writestr('other', '')
        with self.assertRaises(ValueError):
            convert(p, 42, hashlib.sha256(p.read_bytes()).hexdigest())
        with self.assertWarns(UserWarning), self.assertRaises(ValueError):
            self.run_convert(extra=('mapSolarSystems.jsonl', '{}'))
        with patch('convert.time.monotonic', side_effect=[0, 31]), self.assertRaises(ValueError):
            self.run_convert()

    def test_limits_and_provenance(self):
        p = self.archive()
        with self.assertRaises(ValueError):
            convert(p, 42, '0'*64)
        with self.assertRaises(ValueError):
            convert(p, 43, hashlib.sha256(p.read_bytes()).hexdigest())
        with self.assertRaises(ValueError):
            convert(p, -1, hashlib.sha256(p.read_bytes()).hexdigest())
        with patch('convert.MAX_ARCHIVE', 1), self.assertRaises(ValueError):
            self.run_convert()
        with patch('convert.MAX_TOTAL', 1), self.assertRaises(ValueError):
            self.run_convert()
        self.systems = [{'_key': i} for i in range(1, 10002)]
        with self.assertRaises(ValueError):
            self.run_convert()


if __name__ == '__main__':
    unittest.main()
