//! Non-sensitive portable settings. IPC exposes only an allowlisted Appearance value.
//! The same-user process/filesystem is trusted while running; this is not a sandbox against
//! another process replacing directory entries concurrently. Known links/reparse points fail closed.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

const SETTINGS: &str = "settings.json";
const MAX_SETTINGS_BYTES: u64 = 4096;
const MAX_TREE_DEPTH: usize = 64;
const MAX_TREE_ENTRIES: usize = 100_000;
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub preferences: Preferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeId {
    Echelon,
    Amarr,
    Caldari,
    Gallente,
    Minmatar,
}

/// Only the allowlisted theme is writable; no paths or sensitive values.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    #[serde(
        default,
        deserialize_with = "deserialize_theme",
        skip_serializing_if = "Option::is_none"
    )]
    pub theme: Option<ThemeId>,
}

fn deserialize_theme<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ThemeId>, D::Error> {
    ThemeId::deserialize(deserializer).map(Some) // Missing defaults; explicit null/unknown IDs fail.
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            preferences: Preferences::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    UnsafePath,
    Unwritable,
    InvalidSettings,
    UnsupportedVersion,
    Unavailable,
    ChangedSettings,
}

impl StorageError {
    pub fn message(self) -> &'static str {
        match self {
            Self::UnsafePath => "ECHELON OS cannot use this app-data folder. Close the app and use a regular extracted folder without links or junctions. Existing data has not been reset.",
            Self::Unwritable => "ECHELON OS cannot write beside its executable. Close the app and move the complete extracted folder, including app-data, to a writable location. Do not run as administrator. Existing settings have not been reset.",
            Self::InvalidSettings => "ECHELON OS cannot read its settings. Close the app and preserve a backup of app-data before restoring known-good settings. No automatic reset was performed.",
            Self::UnsupportedVersion => "These settings require a different ECHELON OS version. Close the app and use the matching version with a backup of the complete folder. No automatic reset was performed.",
            Self::ChangedSettings => "Settings changed outside this app. Close the app, preserve a backup and restart before saving. Existing data was not overwritten.",
            Self::Unavailable => "ECHELON OS could not initialise portable storage. Close other copies, preserve app-data and retry from a writable extracted folder. No fallback storage was used.",
        }
    }
}

/// Absolute executable location is the only anchor; never consult process CWD or user directories.
pub fn root_for_executable(executable: &Path) -> Result<PathBuf, StorageError> {
    if !executable.is_absolute() {
        return Err(StorageError::UnsafePath);
    }
    let parent = executable.parent().ok_or(StorageError::UnsafePath)?;
    let parent = fs::canonicalize(parent).map_err(|_| StorageError::Unavailable)?;
    Ok(parent.join("app-data"))
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn directory(path: &Path) -> Result<(), StorageError> {
    match fs::create_dir(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err(StorageError::Unwritable),
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| StorageError::Unavailable)?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err(StorageError::UnsafePath);
    }
    Ok(())
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn temporary(root: &Path) -> Result<(TemporaryFile, File), StorageError> {
    // create_new prevents clobbering any pre-existing file, even after a crash or PID reuse.
    for _ in 0..32 {
        let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(".settings-{}-{id}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((TemporaryFile(path), file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(StorageError::Unwritable),
        }
    }
    Err(StorageError::Unavailable)
}

fn check_writable(root: &Path) -> Result<(), StorageError> {
    let (temporary, mut file) = temporary(root)?;
    file.write_all(b"portable-write-check")
        .and_then(|()| file.sync_all())
        .map_err(|_| StorageError::Unwritable)?;
    drop(file);
    fs::remove_file(&temporary.0).map_err(|_| StorageError::Unwritable)?;
    Ok(())
}

fn read_document(root: &Path) -> Result<Option<(Settings, Vec<u8>)>, StorageError> {
    let path = root.join(SETTINGS);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(StorageError::Unavailable),
    };
    if is_link(&metadata) || !metadata.is_file() {
        return Err(StorageError::UnsafePath);
    }
    if metadata.len() > MAX_SETTINGS_BYTES {
        return Err(StorageError::InvalidSettings);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT; do not follow links.
    }
    let file = options.open(path).map_err(|_| StorageError::Unavailable)?;
    let metadata = file.metadata().map_err(|_| StorageError::Unavailable)?;
    if is_link(&metadata) || !metadata.is_file() {
        return Err(StorageError::UnsafePath);
    }
    let mut bytes = Vec::new();
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StorageError::Unavailable)?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(StorageError::InvalidSettings);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| StorageError::InvalidSettings)?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(StorageError::InvalidSettings)?;
    let settings = match version {
        1 => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct LegacyPreferences {}
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct LegacySettings {
                schema_version: u32,
                preferences: LegacyPreferences,
            }
            let legacy: LegacySettings =
                serde_json::from_slice(&bytes).map_err(|_| StorageError::InvalidSettings)?;
            let LegacyPreferences {} = legacy.preferences;
            if legacy.schema_version != 1 {
                return Err(StorageError::InvalidSettings);
            }
            // Read v1 without rewriting it. The first explicit successful theme save upgrades to v2.
            Settings::default()
        }
        2 => serde_json::from_slice(&bytes).map_err(|_| StorageError::InvalidSettings)?,
        _ => return Err(StorageError::UnsupportedVersion),
    };
    Ok(Some((settings, bytes)))
}

/// Publish a completely flushed initial file without ever replacing existing settings.
/// Existing documents are replaced only by the guarded store below.
fn publish_initial(root: &Path, settings: &Settings) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|_| StorageError::InvalidSettings)?;
    let (temporary, mut file) = temporary(root)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| StorageError::Unwritable)?;
    drop(file);
    publish_without_replace(&temporary.0, &root.join(SETTINGS))
        .map_err(|_| StorageError::Unavailable)?;
    Ok(())
}

#[cfg(windows)]
fn publish_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // Same-directory rename, no REPLACE_EXISTING or COPY_ALLOWED. Works without hard links
    // (including writable removable filesystems). NUL-terminated buffers remain alive for the call.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x8) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
fn publish_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    // Atomic create-if-absent for local Unix test/development filesystems.
    fs::hard_link(from, to)?;
    if let Some(parent) = to.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

struct StoredState {
    settings: Settings,
    bytes: Vec<u8>,
}

pub struct PortableStore {
    root: PathBuf,
    inner: Mutex<StoredState>,
    // OS lock is held for this store/app lifetime; released by the OS even on process crash.
    _lock: File,
}

impl PortableStore {
    pub fn open(executable: &Path) -> Result<Self, StorageError> {
        Self::open_with_probe(executable, check_writable)
    }

    fn open_with_probe(
        executable: &Path,
        probe: impl FnOnce(&Path) -> Result<(), StorageError>,
    ) -> Result<Self, StorageError> {
        let root = root_for_executable(executable)?;
        directory(&root)?;
        let lock = lock_store(&root)?;
        let mut entries = 0;
        validate_tree(&root, 0, &mut entries)?;
        directory(&root.join("caches"))?;
        directory(&root.join("logs"))?;
        probe(&root)?;
        let (settings, bytes) = match read_document(&root)? {
            Some(document) => document,
            None => {
                publish_initial(&root, &Settings::default())?;
                read_document(&root)?.ok_or(StorageError::Unavailable)?
            }
        };
        Ok(Self {
            root,
            inner: Mutex::new(StoredState { settings, bytes }),
            _lock: lock,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn theme(&self) -> Result<ThemeId, StorageError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| StorageError::Unavailable)?
            .settings
            .preferences
            .theme
            .unwrap_or(ThemeId::Echelon))
    }

    pub fn set_theme(&self, theme: ThemeId) -> Result<(), StorageError> {
        self.save_with(theme, replace_existing)
    }

    fn save_with(
        &self,
        theme: ThemeId,
        replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<(), StorageError> {
        let mut state = self.inner.lock().map_err(|_| StorageError::Unavailable)?;
        let metadata = fs::symlink_metadata(&self.root).map_err(|_| StorageError::Unavailable)?;
        if is_link(&metadata) || !metadata.is_dir() {
            return Err(StorageError::UnsafePath);
        }
        let (_, current_bytes) = read_document(&self.root)?.ok_or(StorageError::ChangedSettings)?;
        if current_bytes != state.bytes {
            return Err(StorageError::ChangedSettings);
        }
        let target = self.root.join(SETTINGS);
        let metadata = fs::symlink_metadata(&target).map_err(|_| StorageError::Unavailable)?;
        if is_link(&metadata) {
            return Err(StorageError::UnsafePath);
        }
        if metadata.permissions().readonly() {
            return Err(StorageError::Unwritable);
        }
        let next = Settings {
            schema_version: 2,
            preferences: Preferences { theme: Some(theme) },
        };
        let bytes = serde_json::to_vec_pretty(&next).map_err(|_| StorageError::InvalidSettings)?;
        let (temporary, mut file) = temporary(&self.root)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| StorageError::Unwritable)?;
        drop(file);
        // No delete/copy fallback. Failure before commit leaves original document and state intact.
        replace(&temporary.0, &self.root.join(SETTINGS)).map_err(|_| StorageError::Unwritable)?;
        state.settings = next;
        state.bytes = bytes;
        Ok(())
    }
}

fn lock_store(root: &Path) -> Result<File, StorageError> {
    let path = root.join(".settings.lock");
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if is_link(&metadata) || !metadata.is_file() {
            return Err(StorageError::UnsafePath);
        }
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // OPEN_REPARSE_POINT
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000); // O_NOFOLLOW
    }
    let file = options.open(path).map_err(|_| StorageError::Unwritable)?;
    let metadata = file.metadata().map_err(|_| StorageError::Unavailable)?;
    if is_link(&metadata) || !metadata.is_file() {
        return Err(StorageError::UnsafePath);
    }
    file.try_lock().map_err(|_| StorageError::Unavailable)?;
    Ok(file)
}

#[cfg(windows)]
fn replace_existing(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // Same-directory atomic replacement: REPLACE_EXISTING | WRITE_THROUGH, never COPY_ALLOWED.
    // Buffers are NUL-terminated and alive for the call; original is never explicitly deleted.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_existing(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

#[cfg(test)]
fn initialise(executable: &Path) -> Result<(PathBuf, Settings), StorageError> {
    initialise_with_probe(executable, check_writable)
}

#[cfg(test)]
fn initialise_with_probe(
    executable: &Path,
    probe: impl FnOnce(&Path) -> Result<(), StorageError>,
) -> Result<(PathBuf, Settings), StorageError> {
    let store = PortableStore::open_with_probe(executable, probe)?;
    let settings = store
        .inner
        .lock()
        .map_err(|_| StorageError::Unavailable)?
        .settings
        .clone();
    Ok((store.root.clone(), settings))
}

#[cfg(test)]
fn read_settings(root: &Path) -> Result<Option<Settings>, StorageError> {
    read_document(root).map(|document| document.map(|(settings, _)| settings))
}

fn validate_tree(path: &Path, depth: usize, count: &mut usize) -> Result<(), StorageError> {
    if depth > MAX_TREE_DEPTH {
        return Err(StorageError::UnsafePath);
    }
    for entry in fs::read_dir(path).map_err(|_| StorageError::Unavailable)? {
        *count += 1;
        if *count > MAX_TREE_ENTRIES {
            return Err(StorageError::UnsafePath);
        }
        let entry = entry.map_err(|_| StorageError::Unavailable)?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| StorageError::Unavailable)?;
        if is_link(&metadata) {
            return Err(StorageError::UnsafePath);
        }
        if metadata.is_dir() {
            validate_tree(&entry.path(), depth + 1, count)?;
        } else if !metadata.is_file() {
            return Err(StorageError::UnsafePath);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "echelon-portable-test-{}-{}",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("unique test directory");
            Self(path)
        }
        fn exe(&self) -> PathBuf {
            self.0.join("echelon-os.exe")
        }
        fn root(&self) -> PathBuf {
            self.0.join("app-data")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn resolution_uses_absolute_executable_not_working_directory() {
        let f = Fixture::new();
        assert_eq!(
            root_for_executable(&f.exe()).unwrap(),
            fs::canonicalize(&f.0).unwrap().join("app-data")
        );
        assert!(root_for_executable(Path::new("echelon-os.exe")).is_err());
        // No chdir/global process state: the supplied EXE directory differs from test CWD.
        assert_ne!(f.0, std::env::current_dir().unwrap());
    }

    #[test]
    fn first_run_creates_versioned_settings_and_reload_preserves_bytes() {
        let f = Fixture::new();
        let (root, settings) = initialise(&f.exe()).unwrap();
        assert_eq!(settings, Settings::default());
        let bytes = fs::read(root.join(SETTINGS)).unwrap();
        assert_eq!(
            serde_json::from_slice::<Settings>(&bytes).unwrap(),
            settings
        );
        assert_eq!(initialise(&f.exe()).unwrap().1, settings);
        assert_eq!(fs::read(root.join(SETTINGS)).unwrap(), bytes);
        assert_eq!(fs::read_dir(root).unwrap().count(), 4); // settings, logs, caches, OS lock; no probe leftovers
    }

    #[test]
    fn incomplete_staging_file_is_preserved_and_never_read_as_settings() {
        let f = Fixture::new();
        fs::create_dir(f.root()).unwrap();
        let stale = f.root().join(".settings-crashed.tmp");
        fs::write(&stale, b"{incomplete").unwrap();
        initialise(&f.exe()).unwrap();
        assert_eq!(fs::read(&stale).unwrap(), b"{incomplete");
        assert_eq!(read_settings(&f.root()).unwrap(), Some(Settings::default()));
    }

    #[test]
    fn publishing_initial_settings_never_replaces_an_existing_file() {
        let f = Fixture::new();
        fs::create_dir(f.root()).unwrap();
        fs::write(f.root().join(SETTINGS), b"preserve me").unwrap();
        assert!(publish_initial(&f.root(), &Settings::default()).is_err());
        assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), b"preserve me");
    }

    #[test]
    fn malformed_unknown_oversized_and_future_settings_fail_without_reset() {
        for (bytes, expected) in [
            (b"{".to_vec(), StorageError::InvalidSettings),
            (b"{}".to_vec(), StorageError::InvalidSettings),
            (
                br#"{"schema_version":3,"preferences":{}}"#.to_vec(),
                StorageError::UnsupportedVersion,
            ),
            (
                br#"{"schema_version":1,"preferences":{"theme":"invented"}}"#.to_vec(),
                StorageError::InvalidSettings,
            ),
            (
                br#"{"schema_version":1,"schema_version":1,"preferences":{}}"#.to_vec(),
                StorageError::InvalidSettings,
            ),
            (vec![b' '; 4097], StorageError::InvalidSettings),
        ] {
            let f = Fixture::new();
            fs::create_dir(f.root()).unwrap();
            fs::write(f.root().join(SETTINGS), &bytes).unwrap();
            assert_eq!(initialise(&f.exe()).unwrap_err(), expected);
            assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), bytes);
        }
    }

    #[test]
    fn denied_write_probe_stops_initialisation_and_retry_recovers() {
        let f = Fixture::new();
        // Explicit internal seam: reliable on Windows and privileged CI where readonly flags
        // do not represent an ACL-denied directory. Actual folder ACL denial is a manual smoke check.
        let result = initialise_with_probe(&f.exe(), |_| Err(StorageError::Unwritable));
        assert_eq!(result.unwrap_err(), StorageError::Unwritable);
        assert!(!f.root().join(SETTINGS).exists());
        assert!(initialise(&f.exe()).is_ok());
        assert!(!StorageError::Unwritable
            .message()
            .contains(f.0.to_str().unwrap()));
    }

    #[test]
    fn moving_complete_root_reuses_settings_without_absolute_paths() {
        let f = Fixture::new();
        initialise(&f.exe()).unwrap();
        let bytes = fs::read(f.root().join(SETTINGS)).unwrap();
        let destination = Fixture::new();
        let moved = destination.0.join("moved-app");
        fs::rename(&f.0, &moved).unwrap();
        let (root, settings) = initialise(&moved.join("echelon-os.exe")).unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(fs::read(root.join(SETTINGS)).unwrap(), bytes);
        assert!(!String::from_utf8(bytes)
            .unwrap()
            .contains(&f.0.to_string_lossy().to_string()));
    }

    #[test]
    fn file_instead_of_data_directory_fails_closed() {
        let f = Fixture::new();
        fs::write(f.root(), b"not a directory").unwrap();
        assert_eq!(initialise(&f.exe()).unwrap_err(), StorageError::UnsafePath);
        assert_eq!(fs::read(f.root()).unwrap(), b"not a directory");
    }

    #[cfg(unix)]
    #[test]
    fn root_settings_and_nested_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;
        let outside = Fixture::new();
        for location in ["root", "settings", "nested"] {
            let f = Fixture::new();
            if location == "root" {
                symlink(&outside.0, f.root()).unwrap();
            } else {
                fs::create_dir(f.root()).unwrap();
                let name = if location == "settings" {
                    SETTINGS
                } else {
                    "cache-link"
                };
                symlink(&outside.0, f.root().join(name)).unwrap();
            }
            assert_eq!(initialise(&f.exe()).unwrap_err(), StorageError::UnsafePath);
            assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
        }
    }

    #[test]
    fn large_regular_cache_is_valid_and_scan_cost_is_reported() {
        let f = Fixture::new();
        initialise(&f.exe()).unwrap();
        let cache = f.root().join("caches").join("synthetic-profile");
        fs::create_dir(&cache).unwrap();
        let mut previous = 0;
        for count in [1_000, 10_000] {
            for index in previous..count {
                File::create(cache.join(format!("entry-{index}"))).unwrap();
            }
            let start = std::time::Instant::now();
            assert_eq!(initialise(&f.exe()).unwrap().1, Settings::default());
            eprintln!(
                "portable scan: {count} warm synthetic files, {:?}",
                start.elapsed()
            );
            previous = count;
        }
    }

    #[test]
    fn depth_and_entry_budgets_still_fail_closed() {
        let f = Fixture::new();
        fs::create_dir(f.root()).unwrap();
        File::create(f.root().join("file")).unwrap();
        // Seed the counter at the exact boundary rather than create 100,001 files in CI.
        let mut entries = MAX_TREE_ENTRIES;
        assert_eq!(
            validate_tree(&f.root(), 0, &mut entries),
            Err(StorageError::UnsafePath)
        );
        let mut deepest = f.root();
        for _ in 0..=MAX_TREE_DEPTH {
            deepest = deepest.join("d");
            fs::create_dir(&deepest).unwrap();
        }
        assert_eq!(initialise(&f.exe()).unwrap_err(), StorageError::UnsafePath);
    }

    #[cfg(unix)]
    #[test]
    fn link_inside_nested_profile_is_rejected_without_target_writes() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        initialise(&f.exe()).unwrap();
        let profile = f.root().join("caches").join("nested-profile");
        fs::create_dir(&profile).unwrap();
        let outside = Fixture::new();
        symlink(&outside.0, profile.join("nested-link")).unwrap();
        assert_eq!(initialise(&f.exe()).unwrap_err(), StorageError::UnsafePath);
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
    }

    #[test]
    fn v1_load_is_byte_preserving_and_first_save_lazily_migrates() {
        let f = Fixture::new();
        fs::create_dir(f.root()).unwrap();
        let legacy = b"{\n \"schema_version\":1,\n \"preferences\":{}\n}";
        fs::write(f.root().join(SETTINGS), legacy).unwrap();
        {
            let store = PortableStore::open(&f.exe()).unwrap();
            assert_eq!(store.theme().unwrap(), ThemeId::Echelon);
            assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), legacy);
            assert_eq!(
                store.save_with(ThemeId::Caldari, |_, _| Err(std::io::Error::other(
                    "injected migration failure"
                ))),
                Err(StorageError::Unwritable)
            );
            assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), legacy);
            store.set_theme(ThemeId::Caldari).unwrap();
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(f.root().join(SETTINGS)).unwrap()).unwrap();
        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["preferences"]["theme"], "caldari");
        assert_eq!(
            PortableStore::open(&f.exe()).unwrap().theme().unwrap(),
            ThemeId::Caldari
        );
    }

    #[test]
    fn all_allowlisted_ids_save_and_reload_with_no_global_location() {
        for theme in [
            ThemeId::Echelon,
            ThemeId::Amarr,
            ThemeId::Caldari,
            ThemeId::Gallente,
            ThemeId::Minmatar,
        ] {
            let f = Fixture::new();
            {
                let store = PortableStore::open(&f.exe()).unwrap();
                assert_eq!(
                    store.root(),
                    fs::canonicalize(&f.0).unwrap().join("app-data")
                );
                assert_eq!(store.theme().unwrap(), ThemeId::Echelon);
                store.set_theme(theme).unwrap();
            }
            assert_eq!(
                PortableStore::open(&f.exe()).unwrap().theme().unwrap(),
                theme
            );
        }
    }

    #[test]
    fn invalid_persisted_ids_versions_and_legacy_extensions_are_never_reset() {
        for bytes in [
            br#"{"schema_version":2,"preferences":{"theme":"unknown"}}"#.as_slice(),
            br#"{"schema_version":2,"preferences":{"theme":"AMARR"}}"#,
            br#"{"schema_version":2,"preferences":{"theme":null}}"#,
            br#"{"schema_version":1,"preferences":{"theme":"amarr"}}"#,
            br#"{"schema_version":9,"preferences":{}}"#,
            br#"{"schema_version":2,"preferences":{"theme":"amarr","secret":"no"}}"#,
        ] {
            let f = Fixture::new();
            fs::create_dir(f.root()).unwrap();
            fs::write(f.root().join(SETTINGS), bytes).unwrap();
            assert!(PortableStore::open(&f.exe()).is_err());
            assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), bytes);
        }
    }

    #[test]
    fn failed_replace_preserves_prior_bytes_and_cached_selection_then_retry_works() {
        let f = Fixture::new();
        let store = PortableStore::open(&f.exe()).unwrap();
        store.set_theme(ThemeId::Amarr).unwrap();
        let before = fs::read(f.root().join(SETTINGS)).unwrap();
        assert_eq!(
            store.save_with(ThemeId::Gallente, |temporary, destination| {
                assert_eq!(fs::read(destination).unwrap(), before);
                let value: serde_json::Value =
                    serde_json::from_slice(&fs::read(temporary).unwrap()).unwrap();
                assert_eq!(value["preferences"]["theme"], "gallente");
                Err(std::io::Error::other("injected failure before commit"))
            }),
            Err(StorageError::Unwritable)
        );
        assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), before);
        assert_eq!(store.theme().unwrap(), ThemeId::Amarr);
        assert_eq!(fs::read_dir(f.root()).unwrap().count(), 4);
        store.set_theme(ThemeId::Gallente).unwrap();
        assert_eq!(store.theme().unwrap(), ThemeId::Gallente);
    }

    #[test]
    fn external_edit_is_not_overwritten() {
        let f = Fixture::new();
        let store = PortableStore::open(&f.exe()).unwrap();
        let edited = br#"{"schema_version":2,"preferences":{"theme":"minmatar"}}"#;
        fs::write(f.root().join(SETTINGS), edited).unwrap();
        assert_eq!(
            store.set_theme(ThemeId::Amarr),
            Err(StorageError::ChangedSettings)
        );
        assert_eq!(fs::read(f.root().join(SETTINGS)).unwrap(), edited);
    }

    #[test]
    fn os_lock_prevents_second_store_and_releases_on_drop() {
        let f = Fixture::new();
        let store = PortableStore::open(&f.exe()).unwrap();
        assert!(PortableStore::open(&f.exe()).is_err());
        drop(store);
        assert!(PortableStore::open(&f.exe()).is_ok());
    }

    #[test]
    fn concurrent_requests_are_serialized_and_reload_the_committed_value() {
        let f = Fixture::new();
        let store = std::sync::Arc::new(PortableStore::open(&f.exe()).unwrap());
        let mut threads = Vec::new();
        for theme in [
            ThemeId::Amarr,
            ThemeId::Caldari,
            ThemeId::Gallente,
            ThemeId::Minmatar,
        ] {
            let store = store.clone();
            threads.push(std::thread::spawn(move || store.set_theme(theme)));
        }
        for thread in threads {
            thread.join().unwrap().unwrap();
        }
        let expected = store.theme().unwrap();
        drop(store);
        assert_eq!(
            PortableStore::open(&f.exe()).unwrap().theme().unwrap(),
            expected
        );
    }

    #[test]
    fn saved_theme_follows_a_moved_complete_root() {
        let f = Fixture::new();
        {
            let store = PortableStore::open(&f.exe()).unwrap();
            store.set_theme(ThemeId::Minmatar).unwrap();
        }
        let target = Fixture::new();
        let moved = target.0.join("moved-app");
        fs::rename(&f.0, &moved).unwrap();
        assert_eq!(
            PortableStore::open(&moved.join("echelon-os.exe"))
                .unwrap()
                .theme()
                .unwrap(),
            ThemeId::Minmatar
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_rejects_redirected_root_or_settings_without_touching_target() {
        use std::os::unix::fs::symlink;
        for redirect_root in [false, true] {
            let f = Fixture::new();
            let store = PortableStore::open(&f.exe()).unwrap();
            let outside = Fixture::new();
            let target = outside.0.join(SETTINGS);
            let sentinel = br#"{"schema_version":2,"preferences":{"theme":"caldari"}}"#;
            fs::write(&target, sentinel).unwrap();
            if redirect_root {
                fs::rename(f.root(), f.0.join("preserved-data")).unwrap();
                symlink(&outside.0, f.root()).unwrap();
            } else {
                fs::remove_file(f.root().join(SETTINGS)).unwrap();
                symlink(&target, f.root().join(SETTINGS)).unwrap();
            }
            assert_eq!(
                store.set_theme(ThemeId::Amarr),
                Err(StorageError::UnsafePath)
            );
            assert_eq!(fs::read(&target).unwrap(), sentinel);
        }
    }

    #[test]
    fn readonly_settings_are_not_replaced_and_retry_requires_restored_permissions() {
        let f = Fixture::new();
        let store = PortableStore::open(&f.exe()).unwrap();
        let path = f.root().join(SETTINGS);
        let before = fs::read(&path).unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        assert_eq!(
            store.set_theme(ThemeId::Amarr),
            Err(StorageError::Unwritable)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::set_permissions(&path, original).unwrap();
        store.set_theme(ThemeId::Amarr).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_junction_is_rejected() {
        let f = Fixture::new();
        let outside = Fixture::new();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(f.root())
            .arg(&outside.0)
            .status()
            .unwrap();
        assert!(status.success(), "junction test setup must succeed");
        assert_eq!(initialise(&f.exe()).unwrap_err(), StorageError::UnsafePath);
        fs::remove_dir(f.root()).unwrap(); // Remove the junction itself, never its target.
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
    }
}
