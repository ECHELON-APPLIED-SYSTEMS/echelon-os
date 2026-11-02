//! Small invented data only, no actual transport or production pin override.
use super::*;
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    sync::atomic::AtomicUsize,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const SYSTEMS: &str = "{\"_key\":1,\"name\":{\"en\":\"Fiction\"}}\n";
const SNAP: &str = "{\"edges\":[],\"systems\":[[1,\"Fiction\"]],\"version\":1}\n";
struct Fixture {
    path: PathBuf,
    store: super::super::portable::PortableStore,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "echelon-install-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let store =
            super::super::portable::PortableStore::open(&path.join("echelon-os.exe")).unwrap();
        Self { path, store }
    }
    fn root(&self) -> &Path {
        self.store.root()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn archive() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in [
        (
            "_sde.jsonl",
            "{\"_key\":\"sde\",\"buildNumber\":42,\"releaseDate\":\"invented\"}",
        ),
        ("mapSolarSystems.jsonl", SYSTEMS),
        ("mapStargates.jsonl", ""),
    ] {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn with_pin<T>(bytes: &[u8], f: impl FnOnce(&Pin<'_>) -> T) -> T {
    let sha = hash(bytes);
    let snapshot = hash(SNAP.as_bytes());
    let system = hash(SYSTEMS.as_bytes());
    let gates = hash(b"");
    f(&Pin {
        build: 42,
        archive: &sha,
        snapshot: &snapshot,
        tables: [&system, &gates],
        counts: (1, 0, 0, 1),
    })
}
#[test]
fn fake_fetch_to_converter_to_reader_preserves_settings_and_never_overwrites() {
    let f = Fixture::new();
    f.store
        .set_theme(super::super::portable::ThemeId::Amarr)
        .unwrap();
    let settings = fs::read(f.root().join("settings.json")).unwrap();
    let bytes = archive();
    with_pin(&bytes, |pin| {
        let install = Installer::default();
        assert_eq!(
            install.run(f.root(), pin, || Ok(bytes.clone())),
            Ok(Outcome::Installed)
        );
        assert_eq!(navcom::read_root(f.root()).unwrap().unwrap(), SNAP);
        assert_eq!(install.status().unwrap(), Phase::Ready);
        assert_eq!(
            install.run(f.root(), pin, || panic!("already installed must not fetch")),
            Ok(Outcome::AlreadyInstalled)
        );
        assert_eq!(fs::read_dir(f.root().join("navcom")).unwrap().count(), 1);
        fs::write(
            f.root().join("navcom/snapshot.json"),
            "preserve corrupt data",
        )
        .unwrap();
        assert_eq!(
            install.run(f.root(), pin, || panic!(
                "existing corrupt data must not fetch"
            )),
            Err(Error::ExistingData)
        );
        assert_eq!(
            fs::read_to_string(f.root().join("navcom/snapshot.json")).unwrap(),
            "preserve corrupt data"
        );
    });
    assert_eq!(fs::read(f.root().join("settings.json")).unwrap(), settings);
}
#[test]
fn stream_status_size_eof_and_io_errors_are_bounded() {
    for status in [301, 302, 403, 404, 500] {
        assert_eq!(
            read_response(status, None, b"x".as_slice(), 8),
            Err(Error::Response)
        );
    }
    assert_eq!(
        read_response(429, None, b"x".as_slice(), 8),
        Err(Error::RateLimited)
    );
    assert_eq!(
        read_response(200, Some(9), b"x".as_slice(), 8),
        Err(Error::TooLarge)
    );
    assert_eq!(
        read_response(200, None, b"123456789".as_slice(), 8),
        Err(Error::TooLarge)
    );
    assert_eq!(
        read_response(200, Some(2), b"x".as_slice(), 8),
        Err(Error::Truncated)
    );
    assert_eq!(
        read_response(200, None, b"".as_slice(), 8),
        Err(Error::Truncated)
    );
    assert_eq!(
        read_response(200, Some(8), b"12345678".as_slice(), 8).unwrap(),
        b"12345678"
    );
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::TimedOut.into())
        }
    }
    assert_eq!(read_response(200, None, Broken, 8), Err(Error::Network));
    let request = client().unwrap().get(URL).build().unwrap();
    assert_eq!(request.url().as_str(), URL);
    assert!(request.headers().get("authorization").is_none());
    assert_eq!(
        storage::location(Path::new("/repo/target/app-data")),
        Err(Error::Location)
    );
}
#[test]
fn pinned_integrity_and_build_failures_never_publish() {
    let bytes = archive();
    for case in 0..5 {
        let f = Fixture::new();
        with_pin(&bytes, |p| {
            let pin = Pin {
                build: if case == 0 { 43 } else { p.build },
                archive: if case == 1 { "incorrect" } else { p.archive },
                snapshot: if case == 2 { "wrong" } else { p.snapshot },
                tables: if case == 3 {
                    ["wrong", "wrong"]
                } else {
                    p.tables
                },
                counts: if case == 4 { (2, 0, 0, 2) } else { p.counts },
            };
            assert!(Installer::default()
                .run(f.root(), &pin, || Ok(bytes.clone()))
                .is_err());
            assert!(navcom::read_root(f.root()).unwrap().is_none());
            assert_eq!(fs::read_dir(f.root().join("navcom")).unwrap().count(), 0);
        });
    }
}
#[test]
fn single_flight_network_failure_and_second_store_lock() {
    let f = Fixture::new();
    let bytes = archive();
    with_pin(&bytes, |p| {
        let installer = Installer::default();
        assert_eq!(
            installer.run(f.root(), p, || {
                assert_eq!(
                    installer.run(f.root(), p, || panic!("duplicate fetch")),
                    Err(Error::Busy)
                );
                Err(Error::Network)
            }),
            Err(Error::Network)
        );
        assert_eq!(installer.status().unwrap(), Phase::Error);
        assert!(
            super::super::portable::PortableStore::open(&f.path.join("echelon-os.exe")).is_err()
        );
        assert_eq!(
            installer.run(f.root(), p, || Ok(bytes.clone())),
            Ok(Outcome::Installed)
        );
    });
}
#[test]
fn owned_stage_interruption_collision_and_no_clobber() {
    let f = Fixture::new();
    let mut guard = DirectoryGuard::root(f.root()).unwrap();
    let directory = storage::directory(f.root(), &mut guard).unwrap();
    fs::write(directory.join(".route-install-unrelated.tmp"), "unrelated").unwrap();
    {
        let mut stage = storage::Stage::new(&directory, &guard).unwrap();
        stage
            .write(SNAP.as_bytes(), &hash(SNAP.as_bytes()))
            .unwrap();
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    let mut first = storage::Stage::new(&directory, &guard).unwrap();
    first
        .write(SNAP.as_bytes(), &hash(SNAP.as_bytes()))
        .unwrap();
    let mut second = storage::Stage::new(&directory, &guard).unwrap();
    second.write(b"different", &hash(b"different")).unwrap();
    first.publish().unwrap();
    assert_eq!(second.publish(), Err(Error::Publish));
    drop(first);
    drop(second);
    assert_eq!(
        fs::read(directory.join("snapshot.json")).unwrap(),
        SNAP.as_bytes()
    );
    assert_eq!(
        fs::read_to_string(directory.join(".route-install-unrelated.tmp")).unwrap(),
        "unrelated"
    );
    assert_eq!(fs::read_dir(directory).unwrap().count(), 2);
}
#[cfg(unix)]
#[test]
fn links_directory_swaps_and_foreign_cleanup_targets_are_preserved() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let bytes = archive();
    let outside = f.path.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, f.root().join("navcom")).unwrap();
    with_pin(&bytes, |p| {
        assert_eq!(
            Installer::default().run(f.root(), p, || panic!("unsafe path must not fetch")),
            Err(Error::ExistingData)
        )
    });
    fs::remove_file(f.root().join("navcom")).unwrap();
    with_pin(&bytes, |p| {
        assert_eq!(
            Installer::default().run(f.root(), p, || {
                fs::rename(f.root(), f.path.join("moved-data")).unwrap();
                fs::create_dir(f.root()).unwrap();
                Ok(bytes.clone())
            }),
            Err(Error::UnsafePath)
        );
    });
    assert!(!f.root().join("navcom/snapshot.json").exists());
    let root = f.path.join("moved-data");
    let mut guard = DirectoryGuard::root(&root).unwrap();
    let directory = storage::directory(&root, &mut guard).unwrap();
    let stage = storage::Stage::new(&directory, &guard).unwrap();
    let owned = fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::rename(&owned, directory.join("moved-owned.tmp")).unwrap();
    fs::write(&owned, "foreign replacement").unwrap();
    drop(stage);
    assert_eq!(fs::read_to_string(owned).unwrap(), "foreign replacement");
}
#[cfg(windows)]
#[test]
fn windows_guard_blocks_relocation_during_install() {
    let f = Fixture::new();
    let bytes = archive();
    with_pin(&bytes, |p| {
        assert_eq!(
            Installer::default().run(f.root(), p, || {
                assert!(fs::rename(f.root(), f.path.join("moved")).is_err());
                Ok(bytes.clone())
            }),
            Ok(Outcome::Installed)
        )
    });
}

#[test]
fn concurrent_managers_cannot_replace_the_winning_snapshot() {
    let f = Fixture::new();
    let bytes = archive();
    let barrier = std::sync::Barrier::new(2);
    with_pin(&bytes, |pin| {
        std::thread::scope(|scope| {
            let run = || {
                Installer::default().run(f.root(), pin, || {
                    barrier.wait();
                    Ok(bytes.clone())
                })
            };
            let first = scope.spawn(run);
            let second = scope.spawn(run);
            let results = [first.join().unwrap(), second.join().unwrap()];
            assert_eq!(
                results
                    .iter()
                    .filter(|r| **r == Ok(Outcome::Installed))
                    .count(),
                1
            );
            assert_eq!(
                results
                    .iter()
                    .filter(|r| **r == Err(Error::Publish))
                    .count(),
                1
            );
        })
    });
    assert_eq!(navcom::read_root(f.root()).unwrap().unwrap(), SNAP);
    assert_eq!(fs::read_dir(f.root().join("navcom")).unwrap().count(), 1);
}
#[test]
fn readonly_existing_snapshot_is_preserved_without_fetch() {
    let f = Fixture::new();
    fs::create_dir(f.root().join("navcom")).unwrap();
    let path = f.root().join("navcom/snapshot.json");
    fs::write(&path, "preserve").unwrap();
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let bytes = archive();
    with_pin(&bytes, |pin| {
        assert_eq!(
            Installer::default().run(f.root(), pin, || panic!("must not fetch")),
            Err(Error::ExistingData)
        )
    });
    assert_eq!(fs::read_to_string(&path).unwrap(), "preserve");
    assert!(fs::metadata(&path).unwrap().permissions().readonly());
    fs::set_permissions(path, original).unwrap();
}
#[cfg(unix)]
#[test]
fn snapshot_publication_is_private_and_staging_digest_failure_cleans_up() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let mut guard = DirectoryGuard::root(f.root()).unwrap();
    let directory = storage::directory(f.root(), &mut guard).unwrap();
    {
        let mut stage = storage::Stage::new(&directory, &guard).unwrap();
        assert_eq!(stage.write(b"invented", "wrong"), Err(Error::Integrity));
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    let mut stage = storage::Stage::new(&directory, &guard).unwrap();
    stage
        .write(SNAP.as_bytes(), &hash(SNAP.as_bytes()))
        .unwrap();
    stage.publish().unwrap();
    stage.cleanup().unwrap();
    assert_eq!(
        fs::metadata(directory.join("snapshot.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
