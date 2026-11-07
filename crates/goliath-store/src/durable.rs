//! Owns OS durability barriers, including the explicit macOS drive-cache flush.

use std::fs::File;
use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncMode {
    Full,
    Fsync,
}

impl SyncMode {
    pub(crate) fn sync(self, file: &File) -> io::Result<()> {
        match self {
            Self::Fsync => fsync(file),
            Self::Full => full_sync(file),
        }
    }
}

#[cfg(target_os = "macos")]
mod apple {
    //! Owns the small Darwin syscall boundary behind safe file references.

    use std::ffi::c_int;
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;

    unsafe extern "C" {
        fn fsync(fd: c_int) -> c_int;
        fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    }

    pub(super) fn sync(file: &File, full: bool) -> io::Result<()> {
        // SAFETY: file owns a live descriptor for the duration of each syscall.
        // F_FULLFSYNC is Darwin's documented no-argument command 51.
        let result = unsafe {
            if full {
                fcntl(file.as_raw_fd(), 51)
            } else {
                fsync(file.as_raw_fd())
            }
        };
        if result == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

fn fsync(file: &File) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        apple::sync(file, false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        file.sync_all()
    }
}

fn full_sync(file: &File) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        apple::sync(file, true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        file.sync_all()
    }
}

pub(crate) fn sync_directory(path: &std::path::Path, mode: SyncMode) -> io::Result<()> {
    let directory = File::open(path)?;
    fsync(&directory)?;
    // Flush directory metadata as well as file contents. Fail rather than
    // silently downgrade if this filesystem rejects the requested barrier.
    mode.sync(&directory)
}
