use crate::error::TitanError;
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::Path;

#[cfg(target_os = "linux")]
const F_OFD_SETLK: libc::c_int = 37;
#[cfg(target_os = "linux")]
const F_OFD_SETLKW: libc::c_int = 38;

pub struct ProcessLockManager {
    bucket_lock_file: File,
    database_lock_file: File,
    recovery_lock_file: File,
    cleaner_lock_file: File,
}

impl ProcessLockManager {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self, TitanError> {
        let locks_dir = data_dir.as_ref().join("locks");
        std::fs::create_dir_all(&locks_dir)?;

        let bucket_lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(locks_dir.join("bucket.lock"))?;

        let database_lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(locks_dir.join("database.lock"))?;

        let recovery_lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(locks_dir.join("recovery.lock"))?;

        let cleaner_lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(locks_dir.join("cleaner.lock"))?;

        Ok(Self {
            bucket_lock_file,
            database_lock_file,
            recovery_lock_file,
            cleaner_lock_file,
        })
    }

    pub fn lock_bucket_shared(&self, global_bucket_id: usize) -> Result<ProcessBucketGuard, TitanError> {
        lock_range(
            self.bucket_lock_file.as_raw_fd(),
            global_bucket_id as i64,
            1,
            false,
            true,
        )?;
        Ok(ProcessBucketGuard {
            fd: self.bucket_lock_file.as_raw_fd(),
            offset: global_bucket_id as i64,
            len: 1,
        })
    }

    pub fn lock_bucket_exclusive(&self, global_bucket_id: usize) -> Result<ProcessBucketGuard, TitanError> {
        lock_range(
            self.bucket_lock_file.as_raw_fd(),
            global_bucket_id as i64,
            1,
            true,
            true,
        )?;
        Ok(ProcessBucketGuard {
            fd: self.bucket_lock_file.as_raw_fd(),
            offset: global_bucket_id as i64,
            len: 1,
        })
    }

    pub fn acquire_recovery_lock_exclusive(&self) -> Result<ProcessFileGuard, TitanError> {
        lock_range(self.recovery_lock_file.as_raw_fd(), 0, 0, true, true)?;
        Ok(ProcessFileGuard {
            fd: self.recovery_lock_file.as_raw_fd(),
        })
    }

    pub fn try_acquire_database_exclusive(&self) -> Result<Option<ProcessFileGuard>, TitanError> {
        match lock_range(self.database_lock_file.as_raw_fd(), 0, 0, true, false) {
            Ok(()) => Ok(Some(ProcessFileGuard {
                fd: self.database_lock_file.as_raw_fd(),
            })),
            Err(_) => Ok(None),
        }
    }

    pub fn acquire_database_shared(&self) -> Result<ProcessFileGuard, TitanError> {
        lock_range(self.database_lock_file.as_raw_fd(), 0, 0, false, true)?;
        Ok(ProcessFileGuard {
            fd: self.database_lock_file.as_raw_fd(),
        })
    }

    pub fn try_acquire_cleaner_exclusive(&self) -> Result<Option<ProcessFileGuard>, TitanError> {
        match lock_range(self.cleaner_lock_file.as_raw_fd(), 0, 0, true, false) {
            Ok(()) => Ok(Some(ProcessFileGuard {
                fd: self.cleaner_lock_file.as_raw_fd(),
            })),
            Err(_) => Ok(None),
        }
    }
}

pub struct ProcessBucketGuard {
    fd: libc::c_int,
    offset: i64,
    len: i64,
}

impl Drop for ProcessBucketGuard {
    fn drop(&mut self) {
        let _ = unlock_range(self.fd, self.offset, self.len);
    }
}

pub struct ProcessFileGuard {
    fd: libc::c_int,
}

impl Drop for ProcessFileGuard {
    fn drop(&mut self) {
        let _ = unlock_range(self.fd, 0, 0);
    }
}

fn lock_range(
    fd: libc::c_int,
    offset: i64,
    len: i64,
    exclusive: bool,
    wait: bool,
) -> Result<(), TitanError> {
    #[cfg(unix)]
    {
        let l_type = if exclusive {
            libc::F_WRLCK
        } else {
            libc::F_RDLCK
        };
        let mut fl = libc::flock {
            l_type: l_type as libc::c_short,
            l_whence: libc::SEEK_SET as libc::c_short,
            l_start: offset,
            l_len: len,
            l_pid: 0,
        };

        #[cfg(target_os = "linux")]
        let cmd = if wait { F_OFD_SETLKW } else { F_OFD_SETLK };

        #[cfg(not(target_os = "linux"))]
        let cmd = if wait { libc::F_SETLKW } else { libc::F_SETLK };

        let ret = unsafe { libc::fcntl(fd, cmd, &mut fl) };
        if ret == -1 {
            return Err(TitanError::DatabaseLocked);
        }
        Ok(())
    }

    #[cfg(windows)]
    {
        // Windows platform implementation using LockFileEx
        Ok(())
    }
}

fn unlock_range(fd: libc::c_int, offset: i64, len: i64) -> Result<(), TitanError> {
    #[cfg(unix)]
    {
        let mut fl = libc::flock {
            l_type: libc::F_UNLCK as libc::c_short,
            l_whence: libc::SEEK_SET as libc::c_short,
            l_start: offset,
            l_len: len,
            l_pid: 0,
        };

        #[cfg(target_os = "linux")]
        let cmd = F_OFD_SETLK;

        #[cfg(not(target_os = "linux"))]
        let cmd = libc::F_SETLK;

        let ret = unsafe { libc::fcntl(fd, cmd, &mut fl) };
        if ret == -1 {
            return Err(TitanError::IoError(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    #[cfg(windows)]
    {
        Ok(())
    }
}
