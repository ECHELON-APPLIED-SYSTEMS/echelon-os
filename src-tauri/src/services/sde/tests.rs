//! Entirely invented data; ZIPs are constructed in memory, never extracted.
use super::*;
const SYSTEMS:&str="{\"_key\":3}\n{\"_key\":2,\"stargateIDs\":[20]}\n{\"_key\":1,\"name\":{\"en\":\"Fiction é😀\\u007f\"},\"stargateIDs\":[10]}\n";
const GATES:&str="{\"_key\":20,\"solarSystemID\":2,\"destination\":{\"stargateID\":10,\"solarSystemID\":1}}\n{\"_key\":10,\"solarSystemID\":1,\"destination\":{\"stargateID\":20,\"solarSystemID\":2}}\n";
const EXPECTED:&str="{\"edges\":[[1,2]],\"systems\":[[1,\"Fiction \\u00e9\\ud83d\\ude00\\u007f\"],[2,null],[3,null]],\"version\":1}\n";
#[derive(Clone)]
struct Entry {
    name: String,
    data: Vec<u8>,
    mode: u32,
    flags: u16,
    method: u16,
    declared: Option<u32>,
    compressed: Option<u32>,
}
fn entry(name: &str, data: &str) -> Entry {
    Entry {
        name: name.into(),
        data: data.as_bytes().to_vec(),
        mode: 0o100600,
        flags: 0,
        method: 0,
        declared: None,
        compressed: None,
    }
}
fn entries() -> Vec<Entry> {
    vec![
        entry(
            TABLES[0],
            "{\"_key\":\"sde\",\"buildNumber\":42,\"releaseDate\":\"synthetic\"}",
        ),
        entry(TABLES[1], SYSTEMS),
        entry(TABLES[2], GATES),
    ]
}
fn crc(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = (c >> 1) ^ if c & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    !c
}
fn u16b(out: &mut Vec<u8>, n: u16) {
    out.extend(n.to_le_bytes());
}
fn u32b(out: &mut Vec<u8>, n: u32) {
    out.extend(n.to_le_bytes());
}
fn archive(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut dir = Vec::new();
    for e in entries {
        let offset = out.len() as u32;
        let size = e.declared.unwrap_or(e.data.len() as u32);
        let compressed = e.compressed.unwrap_or(e.data.len() as u32);
        out.extend(b"PK\x03\x04");
        u16b(&mut out, 20);
        u16b(&mut out, e.flags);
        u16b(&mut out, e.method);
        u16b(&mut out, 0);
        u16b(&mut out, 33);
        u32b(&mut out, crc(&e.data));
        u32b(&mut out, compressed);
        u32b(&mut out, size);
        u16b(&mut out, e.name.len() as u16);
        u16b(&mut out, 0);
        out.extend(e.name.as_bytes());
        out.extend(&e.data);
        dir.extend(b"PK\x01\x02");
        u16b(&mut dir, 0x314);
        u16b(&mut dir, 20);
        u16b(&mut dir, e.flags);
        u16b(&mut dir, e.method);
        u16b(&mut dir, 0);
        u16b(&mut dir, 33);
        u32b(&mut dir, crc(&e.data));
        u32b(&mut dir, compressed);
        u32b(&mut dir, size);
        u16b(&mut dir, e.name.len() as u16);
        for _ in 0..4 {
            u16b(&mut dir, 0);
        }
        u32b(&mut dir, e.mode << 16);
        u32b(&mut dir, offset);
        dir.extend(e.name.as_bytes());
    }
    let offset = out.len() as u32;
    let size = dir.len() as u32;
    out.extend(dir);
    out.extend(b"PK\x05\x06");
    u16b(&mut out, 0);
    u16b(&mut out, 0);
    u16b(&mut out, entries.len() as u16);
    u16b(&mut out, entries.len() as u16);
    u32b(&mut out, size);
    u32b(&mut out, offset);
    u16b(&mut out, 0);
    out
}
fn run(entries: &[Entry]) -> Result<Projection, Error> {
    let a = archive(entries);
    convert(&a, 42, &digest(&a))
}
#[test]
fn canonical_projection_and_metadata() {
    let a = archive(&entries());
    let result = convert(&a, 42, &digest(&a)).unwrap();
    assert_eq!(result.snapshot, EXPECTED);
    // Independently checked against Python convert.py; no real map/system data.
    assert_eq!(
        result.metadata.snapshot_sha256,
        "c7b670c1645bed915e059f887c05c2256ca98ba57f84d48e7508786d4f587d90"
    );
    assert_eq!(
        (
            result.metadata.systems,
            result.metadata.gates,
            result.metadata.edges,
            result.metadata.isolated_systems
        ),
        (3, 2, 1, 1)
    );
    assert_eq!(
        result.metadata.table_sha256,
        [digest(SYSTEMS.as_bytes()), digest(GATES.as_bytes())]
    );
    assert_eq!(result, convert(&a, 42, &digest(&a)).unwrap());
    let mut reordered = entries();
    reordered.swap(0, 2);
    assert_eq!(run(&reordered).unwrap().snapshot, EXPECTED);
}
#[test]
fn deflated_projection_matches_stored() {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for e in entries() {
        writer
            .start_file(
                e.name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(&e.data).unwrap();
    }
    let a = writer.finish().unwrap().into_inner();
    assert_eq!(convert(&a, 42, &digest(&a)).unwrap().snapshot, EXPECTED);
}
#[test]
fn hashes_build_and_corruption_fail_closed() {
    let a = archive(&entries());
    assert_eq!(convert(&a, 42, &"0".repeat(64)), Err(Error::Digest));
    assert_eq!(convert(&a, 43, &digest(&a)), Err(Error::Build));
    assert_eq!(convert(&a, 0, &digest(&a)), Err(Error::Build));
    let mut bad = a.clone();
    bad[0] ^= 1;
    assert_eq!(convert(&bad, 42, &digest(&a)), Err(Error::Digest));
    let mut bad = a.clone();
    let p = 30 + TABLES[0].len();
    bad[p] ^= 1;
    assert_eq!(convert(&bad, 42, &digest(&bad)), Err(Error::Zip));
    let mut e = entries();
    e[0].data = b"{\"_key\":\"sde\",\"buildNumber\":42.0,\"releaseDate\":\"s\"}".to_vec();
    assert_eq!(run(&e), Err(Error::Build));
}
#[test]
fn all_member_safety_checks_include_unconsumed_files() {
    for name in [
        "../escape",
        "/absolute",
        "a\\b",
        "C:evil",
        "a/../b",
        "a/./b",
        "a//b",
        "dir/",
        "nul\0truncated",
        "",
    ] {
        let mut e = entries();
        e.push(entry(name, "unused"));
        assert!(run(&e).is_err(), "{name:?}");
    }
    for mode in [0o120777, 0o040700, 0o020600, 0o010600] {
        let mut e = entries();
        let mut f = entry("unused", "x");
        f.mode = mode;
        e.push(f);
        assert!(run(&e).is_err());
    }
    let mut e = entries();
    e.push(e[1].clone());
    assert!(run(&e).is_err());
    for method in [12, 99] {
        let mut e = entries();
        let mut f = entry("unused", "x");
        f.method = method;
        e.push(f);
        assert!(run(&e).is_err());
    }
    let mut e = entries();
    let mut f = entry("unused", "x");
    f.flags = 1;
    e.push(f);
    assert!(run(&e).is_err());
    let mut e = entries();
    e.pop();
    assert_eq!(run(&e), Err(Error::Member));
    // A single byte with a huge declared expansion, never actually inflated.
    let mut e = entries();
    let mut f = entry("unused", "x");
    f.declared = Some(1001);
    f.compressed = Some(1);
    e.push(f);
    assert_eq!(run(&e), Err(Error::Limit));
}
#[test]
fn json_types_duplicates_utf8_depth_and_lines() {
    for line in [
        "{\"_key\":1,\"_key\":2}",
        "{\"_key\":1,\"unused\":{\"a\":1,\"a\":2}}",
        "[]",
        "null",
        "{\"_key\":NaN}",
        "{\"_key\":1} {}",
        " ",
    ] {
        let mut e = entries();
        e[1].data = line.as_bytes().to_vec();
        assert!(run(&e).is_err());
    }
    for value in ["0", "-1", "1.0", "true", "\"1\"", "9007199254740992"] {
        let mut e = entries();
        e[1].data = format!("{{\"_key\":{value}}}").into_bytes();
        assert!(run(&e).is_err());
    }
    let mut e = entries();
    e[1].data = vec![0xff];
    assert_eq!(run(&e), Err(Error::Json));
    let mut e = entries();
    e[1].data = format!("{{\"unused\":{}0{}}}", "[".repeat(130), "]".repeat(130)).into_bytes();
    assert_eq!(run(&e), Err(Error::Json));
    let mut e = entries();
    e[1].data = vec![b' '; 65537];
    assert_eq!(run(&e), Err(Error::Limit));
    let mut e = entries();
    e[0].data =
        b"{\"_key\":\"sde\",\"buildNumber\":42,\"buildNumber\":42,\"releaseDate\":\"s\"}".to_vec();
    assert_eq!(run(&e), Err(Error::Json));
}
#[test]
fn topology_and_names_are_never_repaired() {
    for systems in [
        "{\"_key\":1}\n{\"_key\":1}",
        "{\"_key\":1,\"stargateIDs\":[10,10]}\n{\"_key\":2,\"stargateIDs\":[20]}",
        "{\"_key\":1,\"stargateIDs\":[10,30]}\n{\"_key\":2,\"stargateIDs\":[20]}",
        "{\"_key\":1}\n{\"_key\":2,\"stargateIDs\":[20]}",
        "{\"_key\":1,\"name\":null}",
        "{\"_key\":1,\"name\":{\"en\":\"\"}}",
        "{\"_key\":1,\"name\":{\"en\":true}}",
        "{\"_key\":1,\"stargateIDs\":null}",
        "",
    ] {
        let mut e = entries();
        e[1].data = systems.as_bytes().to_vec();
        assert!(run(&e).is_err());
    }
    for gates in [
        GATES.replace("\"stargateID\":10", "\"stargateID\":30"),
        GATES.replace("\"solarSystemID\":2", "\"solarSystemID\":1"),
        GATES.replace("\"_key\":20", "\"_key\":10"),
        GATES.replace("\"solarSystemID\":2", "\"solarSystemID\":99"),
    ] {
        let mut e = entries();
        e[2].data = gates.into_bytes();
        assert!(run(&e).is_err());
    }
    let mut e = entries();
    e[1].data = "{\"_key\":1,\"name\":{\"en\":\"é\"},\"stargateIDs\":[]}"
        .as_bytes()
        .to_vec();
    e[2].data.clear();
    assert_eq!(run(&e).unwrap().metadata.isolated_systems, 1);
}
#[test]
fn every_budget_is_enforced_with_small_fixtures() {
    let a = archive(&entries());
    let pin = digest(&a);
    for case in 0..13 {
        let mut l = LIMITS;
        match case {
            0 => l.archive = a.len() - 1,
            1 => l.members = 2,
            2 => l.total = 1,
            3 => l.member = 1,
            4 => l.ratio = 0,
            5 => l.table = 1,
            6 => l.line = 1,
            7 => l.systems = 2,
            8 => l.gates = 1,
            9 => l.edges = 0,
            10 => l.snapshot = EXPECTED.len() - 1,
            11 => l.time = Duration::ZERO,
            _ => l.line = 10,
        }
        assert!(convert_with(&a, 42, &pin, l).is_err(), "budget {case}");
    }
    let mut l = LIMITS;
    l.snapshot = EXPECTED.len();
    assert_eq!(convert_with(&a, 42, &pin, l).unwrap().snapshot, EXPECTED);
    // Central-directory entry count rejected before library metadata allocation.
    let mut a = a;
    let len = a.len();
    a[len - 14..len - 12].copy_from_slice(&257u16.to_le_bytes());
    a[len - 12..len - 10].copy_from_slice(&257u16.to_le_bytes());
    assert_eq!(convert(&a, 42, &digest(&a)), Err(Error::Limit));
}
#[test]
fn invalid_directory_offsets_and_truncation_do_not_panic() {
    let a = archive(&entries());
    for end in 0..a.len() {
        let b = &a[..end];
        assert!(convert(b, 42, &digest(b)).is_err());
    }
    let mut bad = a;
    let len = bad.len();
    bad[len - 6..len - 2].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(convert(&bad, 42, &digest(&bad)).is_err());
}

#[test]
fn self_links_and_duplicate_system_connections_fail() {
    let mut e = entries();
    e[1].data = br#"{"_key":1,"stargateIDs":[10,20]}"#.to_vec();
    e[2].data=b"{\"_key\":10,\"solarSystemID\":1,\"destination\":{\"stargateID\":20,\"solarSystemID\":1}}\n{\"_key\":20,\"solarSystemID\":1,\"destination\":{\"stargateID\":10,\"solarSystemID\":1}}".to_vec();
    assert_eq!(run(&e), Err(Error::Topology));
    e[1].data =
        b"{\"_key\":1,\"stargateIDs\":[10,11]}\n{\"_key\":2,\"stargateIDs\":[20,21]}".to_vec();
    e[2].data = format!(
        "{GATES}{}",
        GATES.replace(":20", ":21").replace(":10", ":11")
    )
    .into_bytes();
    assert_eq!(run(&e), Err(Error::Topology));
}
#[test]
fn standard_zip64_directory_is_bounded_before_parsing() {
    let mut a = archive(&entries());
    let eocd = a.split_off(a.len() - 22);
    let size = u32::from_le_bytes(eocd[12..16].try_into().unwrap());
    let offset = u32::from_le_bytes(eocd[16..20].try_into().unwrap());
    let record = a.len() as u64;
    a.extend(b"PK\x06\x06");
    a.extend(44u64.to_le_bytes());
    u16b(&mut a, 45);
    u16b(&mut a, 45);
    u32b(&mut a, 0);
    u32b(&mut a, 0);
    a.extend(3u64.to_le_bytes());
    a.extend(3u64.to_le_bytes());
    a.extend(u64::from(size).to_le_bytes());
    a.extend(u64::from(offset).to_le_bytes());
    a.extend(b"PK\x06\x07");
    u32b(&mut a, 0);
    a.extend(record.to_le_bytes());
    u32b(&mut a, 1);
    let mut end = eocd;
    end[8..12].fill(0xff);
    end[12..20].fill(0xff);
    a.extend(end);
    assert_eq!(convert(&a, 42, &digest(&a)).unwrap().snapshot, EXPECTED);
    let p = record as usize + 32;
    a[p..p + 8].copy_from_slice(&257u64.to_le_bytes());
    // The inconsistent per-disk count is rejected before ZIP library allocation.
    assert!(convert(&a, 42, &digest(&a)).is_err());
}
#[test]
fn unrelated_members_are_validated_but_never_decompressed() {
    let mut e = entries();
    let mut unused = entry("unrelated.jsonl", "x");
    unused.method = 8;
    e.push(unused);
    // Deliberately invalid deflate payload is outside the topology projection.
    assert_eq!(run(&e).unwrap().snapshot, EXPECTED);
}
