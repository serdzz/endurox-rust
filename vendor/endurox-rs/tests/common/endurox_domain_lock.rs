//! Exclusive access to the shared Enduro/X integration domain.
//!
//! Every suite is provisioned by `xadmin provision` with the same
//! `NDRX_IPCKEY=44000` and `NDRX_QPREFIX=/test1`, and that key is not a
//! provision template variable, so the suites cannot simply be given distinct
//! domains. They all share one application domain, and two of them running
//! `xadmin start` / `xadmin stop -c -y` at once will tear down each other's
//! servers -- producing a 60-second `TPETIME` and leaving LCF shared-memory
//! residue behind.
//!
//! Two hazards, and they need different mechanisms:
//!
//! * **Across processes.** Cargo runs each integration target as its own
//!   process, in parallel. `flock(2)` on a shared file excludes those.
//!
//! * **Within one process.** Cargo also runs the tests *inside* a binary on
//!   parallel threads, and `flock` does not help there. On macOS and the BSDs a
//!   process holds at most one lock type per file, so a second `flock` from the
//!   same process succeeds instead of blocking -- which is exactly how the two
//!   `02_server_extensions` tests overlapped. A process-local `Mutex` is
//!   required as well.
//!
//! Both are taken here, mutex first so the ordering is uniform and cannot
//! deadlock against another suite.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Held for as long as a test owns the shared Enduro/X domain.
///
/// Field order matters: the `MutexGuard` is declared after the `File` so it is
/// dropped last, releasing the in-process lock only once the cross-process one
/// is gone.
pub struct DomainGuard {
    file: File,
    _local: MutexGuard<'static, ()>,
}

impl Drop for DomainGuard {
    fn drop(&mut self) {
        // Best effort: closing the descriptor releases the lock regardless.
        unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

fn local_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Block until this test has exclusive use of the shared Enduro/X domain.
pub fn lock_endurox_domain() -> DomainGuard {
    // Serialise threads in this process first. A poisoned mutex just means an
    // earlier test panicked; the domain is still ours to take.
    let local = match local_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let path: PathBuf = std::env::temp_dir().join("endurox-rs-integration-domain.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .unwrap_or_else(|err| panic!("failed to open {}: {err}", path.display()));

    // Then exclude the other suites' processes.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        panic!(
            "failed to lock {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
    }

    DomainGuard {
        file,
        _local: local,
    }
}
