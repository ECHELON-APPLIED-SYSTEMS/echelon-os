//! Explicit, read-only fixed-path snapshot access. No startup/provisioning hook.
//! Same-user filesystem trust matches PortableStore; this is not a hostile-process sandbox.
use super::portable::PortableStore;
use serde::Serialize;
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    path::Path,
};

pub const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SnapshotRead {
    Absent,
    Text { text: String },
    Error { code: SnapshotError },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotError {
    UnsafePath,
    Unreadable,
    Oversize,
    InvalidUtf8,
    Changed,
}

pub fn read(store: &PortableStore) -> SnapshotRead {
    match read_root(store.root()) {
        Ok(Some(text)) => SnapshotRead::Text { text },
        Ok(None) => SnapshotRead::Absent,
        Err(code) => SnapshotRead::Error { code },
    }
}
fn link(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_type().is_symlink() || meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len() && a.modified().ok() == b.modified().ok()
    }
}
// Diagnostics exist only in unit-test binaries; no paths or data are accepted.
#[cfg(test)]
thread_local! {
    static FIRST_CHANGED_STAGE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn changed_diagnostic(
    stage: &'static str,
    ancestor_index: Option<usize>,
    before: &Metadata,
    after: Option<&Metadata>,
    read_bytes: Option<usize>,
) {
    FIRST_CHANGED_STAGE.with(|first| {
        if first.get().is_none() {
            first.set(Some(stage));
        }
    });
    eprintln!(
        "NAVCOM_READ_CHANGED stage={stage} ancestor_index={ancestor_index:?} metadata_present={} link={:?} directory={:?} regular_file={:?} size_equal={:?} mtime_equal={:?} identity_equal={:?} read_size_equal={:?}",
        after.is_some(),
        after.map(link),
        after.map(Metadata::is_dir),
        after.map(Metadata::is_file),
        after.map(|m| before.len() == m.len()),
        after.map(|m| before.modified().ok() == m.modified().ok()),
        after.map(|m| same(before, m)),
        after.zip(read_bytes).map(|(m, bytes)| m.len() == bytes as u64),
    );
}
fn metadata(path: &Path) -> Result<Option<Metadata>, SnapshotError> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(SnapshotError::Unreadable),
    }
}
fn open(path: &Path, directory: bool) -> Result<File, SnapshotError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Open the reparse point itself. Retained directory handles deny deletion/
        // rename; the snapshot handle also denies writing for the bounded read.
        options.custom_flags(0x0020_0000 | if directory { 0x0200_0000 } else { 0 });
        options.share_mode(if directory { 1 | 2 } else { 1 });
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NOFOLLOW | O_NONBLOCK: never follow a final link or block on a raced FIFO.
        options.custom_flags(0x20000 | 0x800 | if directory { 0x10000 } else { 0 });
    }
    let file = options.open(path).map_err(|_| SnapshotError::Unreadable)?;
    let m = file.metadata().map_err(|_| SnapshotError::Unreadable)?;
    if link(&m) || (if directory { !m.is_dir() } else { !m.is_file() }) {
        return Err(SnapshotError::UnsafePath);
    }
    Ok(file)
}
fn read_root(root: &Path) -> Result<Option<String>, SnapshotError> {
    read_with_hook(root, || {})
}
fn read_with_hook(
    root: &Path,
    before_open: impl FnOnce(),
) -> Result<Option<String>, SnapshotError> {
    if !root.is_absolute() {
        return Err(SnapshotError::UnsafePath);
    }
    // Only the bounded ancestor chain and two fixed children; never a recursive scan.
    let ancestors: Vec<_> = root.ancestors().collect();
    if ancestors.len() > 128 {
        return Err(SnapshotError::UnsafePath);
    }
    let mut guards = Vec::new();
    for path in ancestors.into_iter().rev() {
        let m = metadata(path)?.ok_or(SnapshotError::UnsafePath)?;
        if link(&m) || !m.is_dir() {
            return Err(SnapshotError::UnsafePath);
        }
        let handle = open(path, true)?;
        let opened_directory = handle.metadata().map_err(|_| SnapshotError::Unreadable)?;
        if !same(&m, &opened_directory) {
            #[cfg(test)]
            changed_diagnostic(
                "ancestor-pre-open",
                Some(guards.len()),
                &m,
                Some(&opened_directory),
                None,
            );
            return Err(SnapshotError::Changed);
        }
        guards.push((path.to_path_buf(), m, handle));
    }
    let directory = root.join("navcom");
    let Some(dm) = metadata(&directory)? else {
        return Ok(None);
    };
    if link(&dm) || !dm.is_dir() {
        return Err(SnapshotError::UnsafePath);
    }
    let dh = open(&directory, true)?;
    let opened_directory = dh.metadata().map_err(|_| SnapshotError::Unreadable)?;
    if !same(&dm, &opened_directory) {
        #[cfg(test)]
        changed_diagnostic(
            "navcom-pre-open",
            Some(guards.len()),
            &dm,
            Some(&opened_directory),
            None,
        );
        return Err(SnapshotError::Changed);
    }
    guards.push((directory.clone(), dm, dh));
    let path = directory.join("snapshot.json");
    let Some(before) = metadata(&path)? else {
        return Ok(None);
    };
    if link(&before) || !before.is_file() {
        return Err(SnapshotError::UnsafePath);
    }
    if before.len() > MAX_BYTES {
        return Err(SnapshotError::Oversize);
    }
    before_open();
    let file = open(&path, false)?;
    let opened = file.metadata().map_err(|_| SnapshotError::Unreadable)?;
    if !same(&before, &opened) {
        #[cfg(test)]
        changed_diagnostic("snapshot-pre-open", None, &before, Some(&opened), None);
        return Err(SnapshotError::Changed);
    }
    if opened.len() > MAX_BYTES {
        return Err(SnapshotError::Oversize);
    }
    let mut bytes = Vec::new();
    (&file)
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SnapshotError::Unreadable)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(SnapshotError::Oversize);
    }
    let after = metadata(&path)?;
    #[cfg(test)]
    if after.is_none() {
        changed_diagnostic(
            "snapshot-post-read-missing",
            None,
            &opened,
            None,
            Some(bytes.len()),
        );
    }
    let after = after.ok_or(SnapshotError::Changed)?;
    if link(&after)
        || !same(&opened, &after)
        || after.len() != bytes.len() as u64
        || opened.modified().ok() != after.modified().ok()
    {
        #[cfg(test)]
        changed_diagnostic(
            "snapshot-post-read",
            None,
            &opened,
            Some(&after),
            Some(bytes.len()),
        );
        return Err(SnapshotError::Changed);
    }
    for (path, prior, _handle) in &guards {
        let now = metadata(path)?;
        #[cfg(test)]
        if now.is_none() {
            changed_diagnostic(
                "ancestor-post-read-missing",
                guards.iter().position(|g| &g.0 == path),
                prior,
                None,
                None,
            );
        }
        let now = now.ok_or(SnapshotError::Changed)?;
        if link(&now) || !now.is_dir() || !same(prior, &now) {
            #[cfg(test)]
            changed_diagnostic(
                "ancestor-post-read",
                guards.iter().position(|g| &g.0 == path),
                prior,
                Some(&now),
                None,
            );
            return Err(SnapshotError::Changed);
        }
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| SnapshotError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "echelon-navcom-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn store(&self) -> PortableStore {
            PortableStore::open(&self.0.join("fiction.exe")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn sanitized_wire_contract() {
        assert_eq!(
            serde_json::to_value(SnapshotRead::Absent).unwrap(),
            serde_json::json!({"kind":"absent"})
        );
        assert_eq!(
            serde_json::to_value(SnapshotRead::Error {
                code: SnapshotError::UnsafePath
            })
            .unwrap(),
            serde_json::json!({"kind":"error","code":"unsafe-path"})
        );
        assert_eq!(
            serde_json::to_value(SnapshotRead::Text {
                text: "fiction".into()
            })
            .unwrap(),
            serde_json::json!({"kind":"text","text":"fiction"})
        );
    }
    #[test]
    fn absent_is_nonfatal_and_never_creates_snapshot_directory() {
        let f = Fixture::new();
        let store = f.store();
        assert_eq!(read(&store), SnapshotRead::Absent);
        assert!(!store.root().join("navcom").exists());
        fs::create_dir(store.root().join("navcom")).unwrap();
        assert_eq!(read(&store), SnapshotRead::Absent);
    }
    #[test]
    fn bounded_utf8_errors_preserve_bytes_and_settings() {
        let f = Fixture::new();
        let store = f.store();
        let settings = fs::read(store.root().join("settings.json")).unwrap();
        let dir = store.root().join("navcom");
        fs::create_dir(&dir).unwrap();
        let path = dir.join("snapshot.json");
        let exact = "x".repeat(MAX_BYTES as usize);
        fs::write(&path, &exact).unwrap();
        assert_eq!(read(&store), SnapshotRead::Text { text: exact });
        for (bytes, code) in [
            (vec![b'x'; MAX_BYTES as usize + 1], SnapshotError::Oversize),
            (vec![255], SnapshotError::InvalidUtf8),
        ] {
            fs::write(&path, &bytes).unwrap();
            assert_eq!(read(&store), SnapshotRead::Error { code });
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::write(&path, b"{malformed").unwrap();
        // Native service transports UTF-8 only. JSON/schema/hash corruption is rejected by TS.
        assert_eq!(
            read(&store),
            SnapshotRead::Text {
                text: "{malformed".into()
            }
        );
        assert_eq!(
            fs::read(store.root().join("settings.json")).unwrap(),
            settings
        );
    }
    #[test]
    fn nonregular_and_relocated_root() {
        let f = Fixture::new();
        {
            let store = f.store();
            fs::create_dir(store.root().join("navcom")).unwrap();
            fs::create_dir(store.root().join("navcom/snapshot.json")).unwrap();
            assert_eq!(
                read(&store),
                SnapshotRead::Error {
                    code: SnapshotError::UnsafePath
                }
            );
            fs::remove_dir(store.root().join("navcom/snapshot.json")).unwrap();
            fs::write(store.root().join("navcom/snapshot.json"), b"fiction").unwrap();
        }
        let target = Fixture::new();
        let moved = target.0.join("moved");
        fs::rename(&f.0, &moved).unwrap();
        let store = PortableStore::open(&moved.join("fiction.exe")).unwrap();
        FIRST_CHANGED_STAGE.with(|first| first.set(None));
        let result = read(&store);
        assert_eq!(
            result,
            SnapshotRead::Text {
                text: "fiction".into()
            },
            "relocated read first_changed_stage={:?}",
            FIRST_CHANGED_STAGE.with(|first| first.get()),
        );
        assert_eq!(FIRST_CHANGED_STAGE.with(|first| first.get()), None);
    }
    #[cfg(unix)]
    #[test]
    fn rejects_links_swaps_and_fifo_without_traversal_or_blocking() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        let outside = Fixture::new();
        let store = f.store();
        let dir = store.root().join("navcom");
        symlink(&outside.0, &dir).unwrap();
        assert_eq!(
            read(&store),
            SnapshotRead::Error {
                code: SnapshotError::UnsafePath
            }
        );
        fs::remove_file(&dir).unwrap();
        fs::create_dir(&dir).unwrap();
        let path = dir.join("snapshot.json");
        symlink(outside.0.join("absent"), &path).unwrap();
        assert_eq!(
            read(&store),
            SnapshotRead::Error {
                code: SnapshotError::UnsafePath
            }
        );
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"original").unwrap();
        FIRST_CHANGED_STAGE.with(|first| first.set(None));
        assert_eq!(
            read_with_hook(store.root(), || {
                fs::rename(&path, dir.join("prior")).unwrap();
                fs::write(&path, b"swapped").unwrap();
            }),
            Err(SnapshotError::Changed)
        );
        assert_eq!(
            FIRST_CHANGED_STAGE.with(|first| first.get()),
            Some("snapshot-pre-open")
        );
        fs::remove_file(&path).unwrap();
        // Avoid spawning a child process while parallel tests hold OS storage locks.
        // The test-only POSIX call creates a FIFO; the production reader never does.
        use std::os::unix::ffi::OsStrExt;
        unsafe extern "C" {
            fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
        }
        let fifo = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert_eq!(
            read(&store),
            SnapshotRead::Error {
                code: SnapshotError::UnsafePath
            }
        );
    }
    #[cfg(unix)]
    #[test]
    fn unreadable_and_growing_file_are_explicit_errors() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new();
        let store = f.store();
        let dir = store.root().join("navcom");
        fs::create_dir(&dir).unwrap();
        let path = dir.join("snapshot.json");
        fs::write(&path, b"fiction").unwrap();
        let prior = fs::metadata(&path).unwrap().permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
        let denied = read(&store);
        fs::set_permissions(&path, prior).unwrap();
        assert_eq!(
            denied,
            SnapshotRead::Error {
                code: SnapshotError::Unreadable
            }
        );
        assert_eq!(
            read_with_hook(store.root(), || {
                OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(MAX_BYTES + 1)
                    .unwrap();
            }),
            Err(SnapshotError::Oversize)
        );
        fs::write(&path, b"fiction").unwrap();
        assert!(read_with_hook(store.root(), || {
            fs::rename(&dir, store.root().join("saved-navcom")).unwrap();
            std::os::unix::fs::symlink(store.root().join("saved-navcom"), &dir).unwrap();
        })
        .is_err());
    }
    #[cfg(windows)]
    #[test]
    fn junction_parent_is_unsafe() {
        let f = Fixture::new();
        let outside = Fixture::new();
        let store = f.store();
        let link = store.root().join("navcom");
        assert!(std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside.0)
            .status()
            .unwrap()
            .success());
        assert_eq!(
            read(&store),
            SnapshotRead::Error {
                code: SnapshotError::UnsafePath
            }
        );
        fs::remove_dir(link).unwrap();
    }
}
