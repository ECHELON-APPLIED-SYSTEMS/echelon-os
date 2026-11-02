//! Pure bounded topology projection. No filesystem, network, extraction or runtime hooks.
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{Cursor, Read},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    ArchiveLimit,
    Digest,
    Zip,
    Member,
    Limit,
    Json,
    Build,
    Topology,
    Time,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Metadata {
    pub build: u64,
    pub archive_sha256: String,
    pub snapshot_sha256: String,
    pub table_sha256: [String; 2],
    pub systems: usize,
    pub gates: usize,
    pub edges: usize,
    pub isolated_systems: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Projection {
    pub snapshot: String,
    pub metadata: Metadata,
}
const MAX_ID: u64 = 9_007_199_254_740_991;
const TABLES: [&str; 3] = ["_sde.jsonl", "mapSolarSystems.jsonl", "mapStargates.jsonl"];
#[derive(Clone, Copy)]
struct Limits {
    archive: usize,
    members: usize,
    total: u64,
    member: u64,
    ratio: u64,
    table: usize,
    line: usize,
    systems: usize,
    gates: usize,
    edges: usize,
    snapshot: usize,
    time: Duration,
}
const LIMITS: Limits = Limits {
    archive: 256 * 1024 * 1024,
    members: 256,
    total: 1024 * 1024 * 1024,
    member: 512 * 1024 * 1024,
    ratio: 1000,
    table: 16 * 1024 * 1024,
    line: 65536,
    systems: 10000,
    gates: 100000,
    edges: 50000,
    snapshot: 4 * 1024 * 1024,
    time: Duration::from_secs(30),
};
struct Budget {
    start: Instant,
    limit: Duration,
}
impl Budget {
    fn check(&self) -> Result<(), Error> {
        if self.start.elapsed() >= self.limit {
            Err(Error::Time)
        } else {
            Ok(())
        }
    }
}
fn require(ok: bool, error: Error) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(error)
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn id(v: Option<&Value>) -> Result<u64, Error> {
    v.and_then(Value::as_u64)
        .filter(|n| *n > 0 && *n <= MAX_ID)
        .ok_or(Error::Topology)
}
/// The caller supplies independently trusted build/hash. Never infer trust from archive contents.
/// Bytes are borrowed immutably and hashed before ZIP parsing; no input copy or disk access.
pub fn convert(
    archive: &[u8],
    expected_build: u64,
    trusted_sha256: &str,
) -> Result<Projection, Error> {
    convert_with(archive, expected_build, trusted_sha256, LIMITS)
}
fn convert_with(
    archive: &[u8],
    build: u64,
    pin: &str,
    limits: Limits,
) -> Result<Projection, Error> {
    let budget = Budget {
        start: Instant::now(),
        limit: limits.time,
    };
    require(build > 0 && build <= MAX_ID, Error::Build)?;
    require(archive.len() <= limits.archive, Error::ArchiveLimit)?;
    require(
        pin.len() == 64
            && pin
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        Error::Digest,
    )?;
    let mut sha = Sha256::new();
    for chunk in archive.chunks(65536) {
        budget.check()?;
        sha.update(chunk);
    }
    let hash = sha
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    require(hash == pin, Error::Digest)?;
    // Bound central-directory counts BEFORE the library can allocate entry metadata.
    let entries = directory_bound(archive, limits, &budget)?;
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|_| Error::Zip)?;
    require(zip.len() == entries, Error::Member)?;
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for n in 0..zip.len() {
        budget.check()?;
        let f = zip.by_index_raw(n).map_err(|_| Error::Zip)?;
        let name = std::str::from_utf8(f.name_raw()).map_err(|_| Error::Member)?;
        require(
            safe_name(name)
                && f.name().map_err(|_| Error::Member)?.as_ref() == name
                && names.insert(name.to_owned()),
            Error::Member,
        )?;
        let mode = f.unix_mode().unwrap_or(0) & 0o170000;
        require(
            !f.is_dir() && !f.encrypted() && (mode == 0 || mode == 0o100000),
            Error::Member,
        )?;
        require(
            matches!(
                f.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ),
            Error::Member,
        )?;
        total = total.checked_add(f.size()).ok_or(Error::Limit)?;
        require(
            total <= limits.total
                && f.size() <= limits.member
                && f.size() <= f.compressed_size().max(1).saturating_mul(limits.ratio),
            Error::Limit,
        )?;
    }
    require(TABLES.iter().all(|n| names.contains(*n)), Error::Member)?;
    let meta = read_member(&mut zip, TABLES[0], limits.line, &budget)?;
    let meta = parse(&meta)?;
    require(
        meta.is_object()
            && meta.get("_key").and_then(Value::as_str) == Some("sde")
            && meta.get("buildNumber").and_then(Value::as_u64) == Some(build)
            && meta.get("releaseDate").is_some_and(Value::is_string),
        Error::Build,
    )?;
    let sys_bytes = read_member(&mut zip, TABLES[1], limits.table, &budget)?;
    let gate_bytes = read_member(&mut zip, TABLES[2], limits.table, &budget)?;
    let mut systems = BTreeMap::new();
    let mut declared = BTreeMap::new();
    let mut declaration_count = 0usize;
    for line in lines(&sys_bytes, limits.line, limits.systems)? {
        budget.check()?;
        let row = parse(line)?;
        require(row.is_object(), Error::Json)?;
        let key = id(row.get("_key"))?;
        require(!systems.contains_key(&key), Error::Topology)?;
        let english = match row.get("name") {
            None => None,
            Some(Value::Object(name)) => match name.get("en") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if !s.is_empty() && s.chars().count() <= 256 => {
                    Some(s.clone())
                }
                _ => return Err(Error::Topology),
            },
            _ => return Err(Error::Topology),
        };
        let mut owned = BTreeSet::new();
        if let Some(list) = row.get("stargateIDs") {
            let list = list.as_array().ok_or(Error::Topology)?;
            require(list.len() <= limits.gates, Error::Limit)?;
            for v in list {
                require(owned.insert(id(Some(v))?), Error::Topology)?;
            }
        }
        declaration_count = declaration_count
            .checked_add(owned.len())
            .ok_or(Error::Limit)?;
        require(declaration_count <= limits.gates, Error::Limit)?;
        systems.insert(key, english);
        declared.insert(key, owned);
    }
    require(!systems.is_empty(), Error::Topology)?;
    let mut gates = BTreeMap::new();
    for line in lines(&gate_bytes, limits.line, limits.gates)? {
        budget.check()?;
        let row = parse(line)?;
        require(row.is_object(), Error::Json)?;
        let key = id(row.get("_key"))?;
        let owner = id(row.get("solarSystemID"))?;
        let dest = row
            .get("destination")
            .filter(|d| d.is_object())
            .ok_or(Error::Topology)?;
        let other = id(dest.get("stargateID"))?;
        let target = id(dest.get("solarSystemID"))?;
        require(
            !gates.contains_key(&key)
                && systems.contains_key(&owner)
                && systems.contains_key(&target)
                && declared.get(&owner).is_some_and(|s| s.contains(&key)),
            Error::Topology,
        )?;
        gates.insert(key, (owner, other, target));
    }
    require(declaration_count == gates.len(), Error::Topology)?;
    let mut edges = BTreeSet::new();
    let mut connected = BTreeSet::new();
    for (&key, &(owner, other, target)) in &gates {
        budget.check()?;
        require(
            owner != target && key != other && gates.get(&other) == Some(&(target, key, owner)),
            Error::Topology,
        )?;
        if key < other {
            require(
                edges.insert((owner.min(target), owner.max(target))),
                Error::Topology,
            )?;
            require(edges.len() <= limits.edges, Error::Limit)?;
            connected.insert(owner);
            connected.insert(target);
        }
    }
    // Fixed lexicographic key order matches Python sort_keys=True. Serialize then
    // escape non-ASCII UTF-16 units (including surrogate pairs) and DEL like Python.
    let rows: Vec<_> = systems
        .iter()
        .map(|(&id, name)| serde_json::json!([id, name]))
        .collect();
    let payload = serde_json::json!({"edges":edges,"systems":rows,"version":1});
    let json = serde_json::to_string(&payload).map_err(|_| Error::Json)?;
    let mut snapshot = String::new();
    for c in json.chars() {
        if c as u32 >= 127 {
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                use std::fmt::Write;
                write!(snapshot, "\\u{u:04x}").map_err(|_| Error::Json)?;
            }
        } else {
            snapshot.push(c);
        }
        require(snapshot.len() < limits.snapshot, Error::Limit)?;
    }
    snapshot.push('\n');
    budget.check()?;
    let metadata = Metadata {
        build,
        archive_sha256: hash,
        snapshot_sha256: digest(snapshot.as_bytes()),
        table_sha256: [digest(&sys_bytes), digest(&gate_bytes)],
        systems: systems.len(),
        gates: gates.len(),
        edges: edges.len(),
        isolated_systems: systems.len() - connected.len(),
    };
    Ok(Projection { snapshot, metadata })
}
fn safe_name(n: &str) -> bool {
    !n.is_empty()
        && !n.contains(['\\', ':', '\0'])
        && !n.starts_with('/')
        && n.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}
fn read_member(
    z: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
    max: usize,
    budget: &Budget,
) -> Result<Vec<u8>, Error> {
    let mut f = z.by_name(name).map_err(|_| Error::Zip)?;
    require(f.size() <= max as u64, Error::Limit)?;
    let size = f.size();
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        budget.check()?;
        let n = f.read(&mut chunk).map_err(|_| Error::Zip)?;
        if n == 0 {
            break;
        }
        require(bytes.len().saturating_add(n) <= max, Error::Limit)?;
        bytes.extend_from_slice(&chunk[..n]);
    }
    require(bytes.len() as u64 == size, Error::Zip)?;
    Ok(bytes)
}
fn lines(bytes: &[u8], max: usize, count: usize) -> Result<Vec<&[u8]>, Error> {
    let mut rows = Vec::new();
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        require(line.len() <= max, Error::Limit)?;
        require(line.iter().any(|b| !b.is_ascii_whitespace()), Error::Json)?;
        require(rows.len() < count, Error::Limit)?;
        rows.push(line);
    }
    Ok(rows)
}
// Retain the normal serde_json depth limit, reject duplicate keys recursively,
// and reject invalid UTF-8, nonfinite numbers and trailing JSON content.
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.into())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element::<Strict>()? {
                    out.push(v.0);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut out = Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    let v = a.next_value::<Strict>()?;
                    out.insert(k, v.0);
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}
fn parse(bytes: &[u8]) -> Result<Value, Error> {
    serde_json::from_slice::<Strict>(bytes)
        .map(|v| v.0)
        .map_err(|_| Error::Json)
}

// Parse only bounded directory framing before delegating ZIP semantics to zip.
// No archive paths are ever extracted. Support ordinary and single-disk ZIP64 EOCD.
fn directory_bound(bytes: &[u8], limits: Limits, budget: &Budget) -> Result<usize, Error> {
    fn u16_at(b: &[u8], p: usize) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            b.get(p..p + 2)
                .ok_or(Error::Zip)?
                .try_into()
                .map_err(|_| Error::Zip)?,
        ))
    }
    fn u32_at(b: &[u8], p: usize) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            b.get(p..p + 4)
                .ok_or(Error::Zip)?
                .try_into()
                .map_err(|_| Error::Zip)?,
        ))
    }
    fn u64_at(b: &[u8], p: usize) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            b.get(p..p + 8)
                .ok_or(Error::Zip)?
                .try_into()
                .map_err(|_| Error::Zip)?,
        ))
    }
    let eocd = (bytes.len().saturating_sub(65557)..bytes.len().saturating_sub(21))
        .rev()
        .find(|&p| {
            bytes.get(p..p + 4) == Some(b"PK\x05\x06")
                && u16_at(bytes, p + 20)
                    .ok()
                    .is_some_and(|n| p + 22 + usize::from(n) == bytes.len())
        })
        .ok_or(Error::Zip)?;
    require(
        u16_at(bytes, eocd + 4)? == 0 && u16_at(bytes, eocd + 6)? == 0,
        Error::Zip,
    )?;
    let mut count = u64::from(u16_at(bytes, eocd + 10)?);
    require(u64::from(u16_at(bytes, eocd + 8)?) == count, Error::Zip)?;
    let mut size = u64::from(u32_at(bytes, eocd + 12)?);
    let mut offset = u64::from(u32_at(bytes, eocd + 16)?);
    let mut end = eocd;
    if eocd >= 20 && bytes.get(eocd - 20..eocd - 16) == Some(b"PK\x06\x07") {
        let loc = eocd - 20;
        require(
            u32_at(bytes, loc + 4)? == 0 && u32_at(bytes, loc + 16)? == 1,
            Error::Zip,
        )?;
        let p = usize::try_from(u64_at(bytes, loc + 8)?).map_err(|_| Error::Zip)?;
        require(
            p <= loc.saturating_sub(56) && bytes.get(p..p + 4) == Some(b"PK\x06\x06"),
            Error::Zip,
        )?;
        require(
            u64_at(bytes, p + 4)? == 44
                && p + 56 == loc
                && u32_at(bytes, p + 16)? == 0
                && u32_at(bytes, p + 20)? == 0,
            Error::Zip,
        )?;
        count = u64_at(bytes, p + 32)?;
        require(u64_at(bytes, p + 24)? == count, Error::Zip)?;
        size = u64_at(bytes, p + 40)?;
        offset = u64_at(bytes, p + 48)?;
        end = p;
    }
    require(count <= limits.members as u64, Error::Limit)?;
    let mut p = usize::try_from(offset).map_err(|_| Error::Zip)?;
    let size = usize::try_from(size).map_err(|_| Error::Zip)?;
    require(p.checked_add(size) == Some(end), Error::Zip)?;
    for _ in 0..count {
        budget.check()?;
        require(bytes.get(p..p + 4) == Some(b"PK\x01\x02"), Error::Zip)?;
        require(u16_at(bytes, p + 34)? == 0, Error::Zip)?;
        let variable = usize::from(u16_at(bytes, p + 28)?)
            + usize::from(u16_at(bytes, p + 30)?)
            + usize::from(u16_at(bytes, p + 32)?);
        p = p
            .checked_add(46 + variable)
            .filter(|p| *p <= end)
            .ok_or(Error::Zip)?;
    }
    require(p == end, Error::Zip)?;
    Ok(count as usize)
}
#[cfg(test)]
mod tests;
