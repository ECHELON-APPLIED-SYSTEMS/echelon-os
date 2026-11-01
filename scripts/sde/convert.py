"""Explicit offline CCP JSONL converter. Never downloads or extracts archive paths."""
import argparse
import hashlib
import json
import io
import os
from pathlib import Path, PurePosixPath
import stat
import time
import zipfile

VERSION = '1'
MAX_ARCHIVE = 256 * 1024 * 1024
MAX_TOTAL = 1024 * 1024 * 1024
MAX_LINE = 65536
TABLES = ('mapSolarSystems.jsonl', 'mapStargates.jsonl')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def ident(value):
    require(type(value) is int and 0 < value <= 9007199254740991, 'invalid ID')
    return value


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encode(value):
    return json.dumps(value, ensure_ascii=True, separators=(',', ':'), sort_keys=True) + '\n'


def convert(archive, build, expected_sha256):
    start = time.monotonic()
    require(type(build) is int and 0 < build <= 9007199254740991, 'invalid build')
    require(archive.stat().st_size <= MAX_ARCHIVE, 'compressed archive limit')
    raw = archive.read_bytes()
    require(digest(raw) == expected_sha256, 'archive SHA256 mismatch')
    tables = {}
    with zipfile.ZipFile(io.BytesIO(raw)) as z:
        infos = z.infolist()
        require(len(infos) <= 256, 'member count limit')
        require(sum(i.file_size for i in infos) <= MAX_TOTAL, 'uncompressed archive limit')
        seen = set()
        for info in infos:
            name = info.filename
            path = PurePosixPath(name)
            mode = info.external_attr >> 16
            require(name and not path.is_absolute() and '..' not in path.parts
                    and '\\' not in name and ':' not in name and '\x00' not in name
                    and name not in seen and name == str(path), 'unsafe or duplicate member')
            require(not info.is_dir() and (stat.S_IFMT(mode) in (0, stat.S_IFREG))
                    and not info.flag_bits & 1 and info.compress_type in (0, 8), 'unsupported member')
            require(info.file_size <= 512 * 1024 * 1024
                    and info.file_size <= max(1, info.compress_size) * 1000, 'inflated member')
            seen.add(name)
        require(all(n in seen for n in (*TABLES, '_sde.jsonl')), 'required table absent')
        require(z.getinfo('_sde.jsonl').file_size <= MAX_LINE, 'build metadata limit')
        meta = json.loads(z.read('_sde.jsonl'), object_pairs_hook=unique_object)
        require(type(meta) is dict and meta.get('_key') == 'sde'
                and type(meta.get('buildNumber')) is int and meta['buildNumber'] == build
                and type(meta.get('releaseDate')) is str, 'archive build mismatch')
        for name in TABLES:
            info = z.getinfo(name)
            require(info.file_size <= 16 * 1024 * 1024, 'table size limit')
            rows = []
            sha = hashlib.sha256()
            with z.open(info) as f:
                while True:
                    require(time.monotonic() - start < 30, 'conversion time budget')
                    line = f.readline(MAX_LINE + 1)
                    if not line:
                        break
                    require(len(line) <= MAX_LINE and line.strip(), 'invalid or oversized line')
                    sha.update(line)
                    row = json.loads(line, object_pairs_hook=unique_object,
                                     parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))
                    require(type(row) is dict, 'invalid row')
                    rows.append(row)
                    require(len(rows) <= (10000 if name == TABLES[0] else 100000), 'row limit')
            tables[name] = (rows, sha.hexdigest())
    systems = {}
    declared = {}
    for row in tables[TABLES[0]][0]:
        key = ident(row.get('_key'))
        require(key not in systems, 'duplicate system')
        name = row.get('name', {})
        require(type(name) is dict, 'invalid localized name')
        english = name.get('en')
        require(english is None or (type(english) is str and 0 < len(english) <= 256), 'invalid English name')
        gates = row.get('stargateIDs', [])
        require(type(gates) is list and len(gates) <= 100000, 'invalid gate list')
        require(all(ident(g) for g in gates) and len(set(gates)) == len(gates), 'duplicate gate declaration')
        systems[key] = [key, english]
        declared[key] = set(gates)
    require(systems, 'empty systems table')
    gates = {}
    for row in tables[TABLES[1]][0]:
        key = ident(row.get('_key'))
        system = ident(row.get('solarSystemID'))
        dest = row.get('destination')
        require(type(dest) is dict, 'invalid destination')
        destination = (ident(dest.get('stargateID')), ident(dest.get('solarSystemID')))
        require(key not in gates and system in systems and destination[1] in systems, 'duplicate gate or unknown system')
        require(key in declared[system], 'undeclared gate')
        gates[key] = (system, destination)
    require(sum(len(s) for s in declared.values()) == len(gates), 'incomplete system gate declarations')
    edges = set()
    for key, (system, (other, target)) in gates.items():
        require(system != target and other != key and gates.get(other) == (target, (key, system)), 'nonreciprocal gate')
        edge = tuple(sorted((system, target)))
        if key < other:
            require(edge not in edges, 'duplicate system connection')
            edges.add(edge)
    require(len(edges) <= 50000, 'edge limit')
    payload = {'version': 1, 'systems': [systems[k] for k in sorted(systems)],
               'edges': sorted(edges)}
    snapshot = encode(payload)
    require(len(snapshot.encode()) <= 4 * 1024 * 1024, 'snapshot byte limit')
    require(time.monotonic() - start < 30, 'conversion time budget')
    metadata = {'version': 1, 'converterVersion': VERSION, 'build': build,
                'sourceUrl': f'https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-{build}-jsonl.zip',
                'archiveSha256': expected_sha256, 'snapshotSha256': digest(snapshot.encode()),
                'tableSha256': {n: tables[n][1] for n in TABLES},
                'systems': len(systems), 'gates': len(gates), 'edges': len(edges),
                'isolatedSystems': len(set(systems) - {s for e in edges for s in e})}
    return snapshot, encode(metadata)


def write_output(directory, snapshot, metadata):
    # Exclusive directory creation: never overwrite a prior snapshot or follow a symlink.
    directory.mkdir(mode=0o700, parents=False, exist_ok=False)
    for name, data in [('snapshot.json', snapshot), ('provenance.json', metadata)]:
        with open(directory / name, 'x', encoding='utf-8') as f:
            os.chmod(f.name, 0o600)
            f.write(data)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--archive', type=Path, required=True)
    p.add_argument('--build', type=int, required=True)
    p.add_argument('--sha256', required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    # Stronger than ignore rules: production outputs cannot be inside this checkout.
    repo = Path(__file__).resolve().parents[2]
    require(not a.output.resolve().is_relative_to(repo)
            and not a.archive.resolve().is_relative_to(repo), 'CCP data must be outside checkout')
    snapshot, metadata = convert(a.archive, a.build, a.sha256)
    write_output(a.output, snapshot, metadata)
    print(metadata, end='')  # metadata only; never game data


if __name__ == '__main__':
    main()
