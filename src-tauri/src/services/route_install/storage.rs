use super::super::navcom::DirectoryGuard;
use super::{hash, Error};
#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub(super) fn location(root: &Path) -> Result<(), Error> {
    // Production downloads are Windows release-only. No developer/CI target may
    // fetch game data; tests call the injected pipeline with fabricated bytes.
    if cfg!(test) || cfg!(debug_assertions) || !cfg!(windows) {
        return Err(Error::Location);
    }
    for p in root.ancestors() {
        if fs::symlink_metadata(p.join(".git")).is_ok() {
            return Err(Error::Location);
        }
        if forbidden_name(p) {
            return Err(Error::Location);
        }
    }
    for variable in [
        "OneDrive",
        "OneDriveConsumer",
        "OneDriveCommercial",
        "TEMP",
        "TMP",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        if std::env::var_os(variable)
            .and_then(|p| fs::canonicalize(p).ok())
            .is_some_and(|p| root.starts_with(p))
        {
            return Err(Error::Location);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::path::{Component, Prefix};
        let Some(Component::Prefix(prefix)) = root.components().next() else {
            return Err(Error::Location);
        };
        let drive = match prefix.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => d,
            _ => return Err(Error::Location),
        };
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetDriveTypeW(root: *const u16) -> u32;
        }
        let path = format!("{}:\\", drive as char);
        let text: Vec<u16> = std::ffi::OsStr::new(&path)
            .encode_wide()
            .chain(Some(0))
            .collect();
        if !matches!(unsafe { GetDriveTypeW(text.as_ptr()) }, 2 | 3) {
            return Err(Error::Location);
        }
    }
    Ok(())
}
fn forbidden_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name == "target"
                || name == "appdata"
                || [
                    "onedrive",
                    "dropbox",
                    "google drive",
                    "box sync",
                    "icloud drive",
                ]
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
}
pub(super) fn directory(root: &Path, guard: &mut DirectoryGuard) -> Result<PathBuf, Error> {
    guard.verify().map_err(|_| Error::UnsafePath)?;
    let dir = root.join("navcom");
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    match builder.create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(Error::Storage),
    }
    guard.hold(&dir).map_err(|_| Error::UnsafePath)?;
    Ok(dir)
}
pub(super) struct Stage<'a> {
    file: File,
    path: PathBuf,
    guard: &'a DirectoryGuard,
    published: bool,
}
impl<'a> Stage<'a> {
    pub(super) fn new(dir: &Path, guard: &'a DirectoryGuard) -> Result<Self, Error> {
        guard.verify().map_err(|_| Error::UnsafePath)?;
        let mut random = [0u8; 24];
        getrandom::fill(&mut random).map_err(|_| Error::Storage)?;
        let name = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let path = dir.join(format!(".route-install-{name}.tmp"));
        #[cfg(not(windows))]
        let file = {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600).custom_flags(0x20000);
            }
            options.open(&path).map_err(|_| Error::Storage)?
        };
        #[cfg(windows)]
        let file = private_file(&path)?;
        let stage = Self {
            file,
            path,
            guard,
            published: false,
        };
        stage.guard.verify().map_err(|_| Error::UnsafePath)?;
        Ok(stage)
    }
    pub(super) fn write(&mut self, bytes: &[u8], pin: &str) -> Result<(), Error> {
        self.guard.verify().map_err(|_| Error::UnsafePath)?;
        self.file
            .write_all(bytes)
            .and_then(|()| self.file.sync_all())
            .map_err(|_| Error::Storage)?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Storage)?;
        let mut read = Vec::new();
        (&self.file)
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut read)
            .map_err(|_| Error::Storage)?;
        if read.len() != bytes.len() || hash(&read) != pin {
            return Err(Error::Integrity);
        }
        self.guard.verify().map_err(|_| Error::UnsafePath)
    }
    pub(super) fn publish(&mut self) -> Result<(), Error> {
        self.guard.verify().map_err(|_| Error::UnsafePath)?;
        promote(&self.file, &self.path, self.guard.directory())?;
        self.published = true;
        self.guard.verify().map_err(|_| Error::UnsafePath)?;
        #[cfg(unix)]
        self.guard
            .directory()
            .sync_all()
            .map_err(|_| Error::Storage)?;
        Ok(())
    }
}
#[cfg(target_os = "linux")]
fn promote(file: &File, _path: &Path, directory: &File) -> Result<(), Error> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn linkat(
            oldfd: i32,
            old: *const std::ffi::c_char,
            newfd: i32,
            new: *const std::ffi::c_char,
            flags: i32,
        ) -> i32;
    }
    // Link the actual held inode (not a replaceable staging pathname). Procfs is
    // kernel-owned; unsupported procfs/hard-link filesystems fail closed.
    let source = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))
        .map_err(|_| Error::Storage)?;
    let result = unsafe {
        linkat(
            -100,
            source.as_ptr(),
            directory.as_raw_fd(),
            c"snapshot.json".as_ptr(),
            0x400,
        )
    };
    if result != 0 {
        return Err(Error::Publish);
    }
    Ok(())
}
#[cfg(windows)]
fn promote(file: &File, path: &Path, _directory: &File) -> Result<(), Error> {
    use std::os::{windows::ffi::OsStrExt, windows::io::AsRawHandle};
    #[repr(C)]
    struct Rename {
        replace: u8,
        root: *mut std::ffi::c_void,
        length: u32,
        name: [u16; 1],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            file: *mut std::ffi::c_void,
            class: i32,
            info: *mut std::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    let target = path.parent().ok_or(Error::Storage)?.join("snapshot.json");
    let name: Vec<u16> = target.as_os_str().encode_wide().collect();
    let offset = std::mem::offset_of!(Rename, name);
    let size = offset + name.len() * 2;
    let mut words = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    let raw = words.as_mut_ptr().cast::<Rename>();
    // Aligned variable-size FILE_RENAME_INFO. ReplaceIfExists remains FALSE.
    // Rename the exclusively held source handle with DELETE access, not a path.
    unsafe {
        (*raw).replace = 0;
        (*raw).root = std::ptr::null_mut();
        (*raw).length = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            words.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            name.len(),
        );
        if SetFileInformationByHandle(file.as_raw_handle(), 3, raw.cast(), size as u32) == 0 {
            return Err(Error::Publish);
        }
    }
    Ok(())
}
#[cfg(not(any(windows, target_os = "linux")))]
fn promote(_file: &File, _path: &Path, _directory: &File) -> Result<(), Error> {
    Err(Error::Publish)
}
impl Stage<'_> {
    pub(super) fn cleanup(&mut self) -> Result<(), Error> {
        #[cfg(windows)]
        {
            if self.published {
                return Ok(());
            }
            use std::os::windows::io::AsRawHandle;
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn SetFileInformationByHandle(
                    file: *mut std::ffi::c_void,
                    class: i32,
                    info: *mut std::ffi::c_void,
                    size: u32,
                ) -> i32;
            }
            let mut delete = 1u8;
            if unsafe {
                SetFileInformationByHandle(
                    self.file.as_raw_handle(),
                    4,
                    (&mut delete as *mut u8).cast(),
                    1,
                )
            } == 0
            {
                return Err(Error::Storage);
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.guard.verify().map_err(|_| Error::UnsafePath)?;
            let held = self.file.metadata().map_err(|_| Error::Storage)?;
            let current = match fs::symlink_metadata(&self.path) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) => return Err(Error::Storage),
            };
            if !current.is_file()
                || current.file_type().is_symlink()
                || held.dev() != current.dev()
                || held.ino() != current.ino()
            {
                return Err(Error::UnsafePath);
            }
            fs::remove_file(&self.path).map_err(|_| Error::Storage)?;
        }
        Ok(())
    }
}
impl Drop for Stage<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(windows)]
fn private_file(path: &Path) -> Result<File, Error> {
    use std::ffi::c_void;
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    #[repr(C)]
    struct Security {
        length: u32,
        descriptor: *mut c_void,
        inherit: i32,
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text: *const u16,
            revision: u32,
            out: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            path: *const u16,
            access: u32,
            share: u32,
            security: *const Security,
            creation: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    // Protected owner-rights DACL: do not inherit a broadly readable folder ACL.
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)".encode_utf16().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(Error::Storage);
    }
    let security = Security {
        length: std::mem::size_of::<Security>() as u32,
        descriptor,
        inherit: 0,
    };
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // CREATE_NEW, no sharing, read/write + DELETE, no-follow final component.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            0xC0010000,
            0,
            &security,
            1,
            0x00200080,
            std::ptr::null_mut(),
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    if handle == (-1isize as *mut c_void) {
        return Err(Error::Storage);
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_private_location_exclusions_are_case_insensitive() {
        for name in [
            "target",
            "AppData",
            "OneDrive - Example",
            "DROPBOX",
            "Google Drive",
            "Box Sync",
            "iCloud Drive",
        ] {
            assert!(forbidden_name(Path::new(name)));
        }
        assert!(!forbidden_name(Path::new("ECHELON-OS")));
    }
}
