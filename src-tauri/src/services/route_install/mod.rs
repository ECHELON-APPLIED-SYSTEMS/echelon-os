//! Explicit fixed-source first installation. No startup downloads or credential access.
mod storage;
#[cfg(test)]
mod tests;
use super::{
    navcom::{self, DirectoryGuard},
    sde,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
const URL:&str="https://developers.eveonline.com/static-data/tranquility/eve-online-static-data-3662168-jsonl.zip";
const ARCHIVE: &str = "2114f82f41c390f3df43d6f94277ac6d658c03f13c7e255dcbe3d86e5a523751";
const SNAPSHOT: &str = "61c6e401dd97e52d4c236ed5dd16920a38dfa9c76fc637a17dc8a2e90ab9fc3b";
const TABLES: [&str; 2] = [
    "d2ea980c977a4818938bf3d9238608660e6685149d28139aa7749841e80ed7c7",
    "2e1185669643d7a4454cd0a88be394dd67b9d8a3d95d028638350a8d78d16d5b",
];
const MAX: usize = 256 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    Busy,
    Location,
    UnsafePath,
    ExistingData,
    Network,
    RateLimited,
    Response,
    TooLarge,
    Truncated,
    Integrity,
    Conversion,
    Storage,
    Publish,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Idle,
    Downloading,
    Verifying,
    Installing,
    Ready,
    Error,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Installed,
    AlreadyInstalled,
}
struct Inner {
    busy: AtomicBool,
    phase: Mutex<Phase>,
}
#[derive(Clone)]
pub struct Installer(Arc<Inner>);
impl Default for Installer {
    fn default() -> Self {
        Self(Arc::new(Inner {
            busy: AtomicBool::new(false),
            phase: Mutex::new(Phase::Idle),
        }))
    }
}
struct Flight<'a>(&'a AtomicBool);
impl Drop for Flight<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
struct Pin<'a> {
    build: u64,
    archive: &'a str,
    snapshot: &'a str,
    tables: [&'a str; 2],
    counts: (usize, usize, usize, usize),
}
const PIN: Pin<'static> = Pin {
    build: 3662168,
    archive: ARCHIVE,
    snapshot: SNAPSHOT,
    tables: TABLES,
    counts: (8490, 13978, 6989, 3222),
};
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
impl Installer {
    pub fn status(&self) -> Result<Phase, Error> {
        self.0.phase.lock().map(|p| *p).map_err(|_| Error::Storage)
    }
    fn phase(&self, p: Phase) -> Result<(), Error> {
        *self.0.phase.lock().map_err(|_| Error::Storage)? = p;
        Ok(())
    }
    pub fn install(&self, root: &Path) -> Result<Outcome, Error> {
        storage::location(root)?;
        self.run(root, &PIN, fetch)
    }
    fn run(
        &self,
        root: &Path,
        pin: &Pin<'_>,
        fetch: impl FnOnce() -> Result<Vec<u8>, Error>,
    ) -> Result<Outcome, Error> {
        self.0
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Error::Busy)?;
        let _flight = Flight(&self.0.busy);
        let result = (|| {
            let mut guard = DirectoryGuard::root(root).map_err(|_| Error::UnsafePath)?;
            // Existing reader is authoritative; never repair or delete any existing data.
            match navcom::read_root(root) {
                Ok(Some(text)) if hash(text.as_bytes()) == pin.snapshot => {
                    self.phase(Phase::Ready)?;
                    return Ok(Outcome::AlreadyInstalled);
                }
                Ok(None) => {}
                _ => return Err(Error::ExistingData),
            }
            let directory = storage::directory(root, &mut guard)?;
            self.phase(Phase::Downloading)?;
            let bytes = fetch()?;
            guard.verify().map_err(|_| Error::UnsafePath)?;
            self.phase(Phase::Verifying)?;
            let p = sde::convert(&bytes, pin.build, pin.archive).map_err(|e| {
                if e == sde::Error::Digest {
                    Error::Integrity
                } else {
                    Error::Conversion
                }
            })?;
            drop(bytes); // Raw archive is memory-only; never staged/persisted.
            if p.metadata.snapshot_sha256 != pin.snapshot
                || hash(p.snapshot.as_bytes()) != pin.snapshot
                || p.metadata.table_sha256 != pin.tables
                || (
                    p.metadata.systems,
                    p.metadata.gates,
                    p.metadata.edges,
                    p.metadata.isolated_systems,
                ) != pin.counts
            {
                return Err(Error::Integrity);
            }
            self.phase(Phase::Installing)?;
            let mut stage = storage::Stage::new(&directory, &guard)?;
            stage.write(p.snapshot.as_bytes(), pin.snapshot)?;
            stage.publish()?;
            stage.cleanup()?;
            drop(stage);
            match navcom::read_root(root) {
                Ok(Some(text)) if hash(text.as_bytes()) == pin.snapshot => {}
                _ => return Err(Error::Integrity),
            }
            self.phase(Phase::Ready)?;
            Ok(Outcome::Installed)
        })();
        if result.is_err() {
            let _ = self.phase(Phase::Error);
        }
        result
    }
}
fn client() -> Result<reqwest::blocking::Client, Error> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .user_agent("ECHELON-OS/0.1 (https://github.com/ECHELON-APPLIED-SYSTEMS/echelon-os)")
        .build()
        .map_err(|_| Error::Network)
}
fn fetch() -> Result<Vec<u8>, Error> {
    let mut response = client()?
        .get(URL)
        .header("Accept", "application/zip")
        .send()
        .map_err(|_| Error::Network)?;
    let status = response.status().as_u16();
    let length = response.content_length();
    read_response(status, length, &mut response, MAX)
}
fn read_response(
    status: u16,
    length: Option<u64>,
    mut reader: impl Read,
    max: usize,
) -> Result<Vec<u8>, Error> {
    if status == 429 {
        return Err(Error::RateLimited);
    }
    if status != 200 {
        return Err(Error::Response);
    }
    if length.is_some_and(|n| n > max as u64) {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 32768];
    loop {
        let n = reader.read(&mut buffer).map_err(|_| Error::Network)?;
        if n == 0 {
            break;
        }
        if bytes.len().saturating_add(n) > max {
            return Err(Error::TooLarge);
        }
        if bytes.len() + n > bytes.capacity() {
            let capacity = bytes.capacity().max(65536).saturating_mul(2).min(max);
            bytes
                .try_reserve_exact(capacity - bytes.len())
                .map_err(|_| Error::TooLarge)?;
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    if bytes.is_empty() || length.is_some_and(|n| n != bytes.len() as u64) {
        return Err(Error::Truncated);
    }
    Ok(bytes)
}
