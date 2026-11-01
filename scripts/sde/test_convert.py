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

    def literal_member(self, name):
        # Patch a disposable ASCII name, bypassing zipfile's writer normalization.
        # Same lengths preserve offsets, sizes and CRCs (only names change).
        encoded = name.encode('ascii')
        self.assertEqual(len(encoded), 3)
        p = self.archive(extra=('a_b', 'x'))
        with zipfile.ZipFile(p) as z:
            header = z.getinfo('a_b').header_offset
        raw = p.read_bytes()
        self.assertEqual(raw.count(b'a_b'), 2)
        local_name = header + 30
        central_name = raw.index(b'a_b', local_name + 3)
        self.assertEqual(raw[header:header + 4], b'PK\x03\x04')
        self.assertEqual(int.from_bytes(raw[header + 26:header + 28], 'little'), 3)
        self.assertEqual(raw[central_name - 46:central_name - 42], b'PK\x01\x02')
        self.assertEqual(int.from_bytes(raw[central_name - 18:central_name - 16], 'little'), 3)
        patched = raw.replace(b'a_b', encoded)
        self.assertEqual(patched[local_name:local_name + 3], encoded)
        self.assertEqual(patched[central_name:central_name + 3], encoded)
        p.write_bytes(patched)
        with zipfile.ZipFile(p) as z:
            self.assertEqual(z.infolist()[-1].orig_filename, name)
        return p

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
            with self.subTest(name=name):
                p = self.literal_member(name) if '\\' in name else self.archive(extra=(name, 'x'))
                with zipfile.ZipFile(p) as z:
                    self.assertEqual(z.infolist()[-1].orig_filename, name)
                with self.assertRaises(ValueError):
                    convert(p, 42, hashlib.sha256(p.read_bytes()).hexdigest())
        info = zipfile.ZipInfo('link')
        info.create_system = 3
        info.external_attr = (stat.S_IFLNK | 0o777) << 16
        with self.assertRaises(ValueError):
            self.run_convert(extra=(info, 'target'))

    def test_literal_name_normalization_is_rejected(self):
        for original, normalized in [('a\\b', 'a/b'), ('a\x00b', 'a')]:
            with self.subTest(original=original):
                p = self.literal_member(original)
                sha = hashlib.sha256(p.read_bytes()).hexdigest()
                # Inspect original bytes before expecting rejection on the host.
                with self.assertRaises(ValueError):
                    convert(p, 42, sha)
                # Exercise CPython's Windows separator path; not native Windows execution.
                with patch.object(zipfile.os, 'sep', '\\'), patch.object(zipfile.os, 'altsep', '/'):
                    with zipfile.ZipFile(p) as z:
                        info = z.infolist()[-1]
                        self.assertEqual(info.orig_filename, original)
                        self.assertEqual(info.filename, normalized)
                    with self.assertRaisesRegex(ValueError, 'normalized member name'):
                        convert(p, 42, sha)

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
