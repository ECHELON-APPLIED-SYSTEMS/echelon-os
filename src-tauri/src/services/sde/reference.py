"""Development-only Python oracle for Rust's invented conformance fixture.

Run from any directory. No production data, native runtime dependency or network.
Only a temporary fabricated archive is written, and removed before exit.
"""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import zipfile

repo = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('reviewed_sde', repo / 'scripts/sde/convert.py')
reference = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference)
systems = ('{"_key":3}\n{"_key":2,"stargateIDs":[20]}\n'
           '{"_key":1,"name":{"en":"Fiction é😀\\u007f"},"stargateIDs":[10]}\n')
gates = ('{"_key":20,"solarSystemID":2,"destination":{"stargateID":10,"solarSystemID":1}}\n'
         '{"_key":10,"solarSystemID":1,"destination":{"stargateID":20,"solarSystemID":2}}\n')
expected = ('{"edges":[[1,2]],"systems":[[1,"Fiction \\u00e9\\ud83d\\ude00\\u007f"],'
            '[2,null],[3,null]],"version":1}\n')
with tempfile.TemporaryDirectory(prefix='echelon-synthetic-sde-') as directory:
    archive = Path(directory) / 'fabricated.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_STORED) as output:
        output.writestr('_sde.jsonl', '{"_key":"sde","buildNumber":42,"releaseDate":"synthetic"}')
        output.writestr('mapSolarSystems.jsonl', systems)
        output.writestr('mapStargates.jsonl', gates)
    snapshot, _ = reference.convert(archive, 42, hashlib.sha256(archive.read_bytes()).hexdigest())
    assert snapshot == expected
    assert hashlib.sha256(snapshot.encode()).hexdigest() == 'c7b670c1645bed915e059f887c05c2256ca98ba57f84d48e7508786d4f587d90'
print('Python oracle matches Rust fixture constant: 99 canonical ASCII bytes, build 42, 3 systems / 2 gates / 1 edge / 1 isolated.')
