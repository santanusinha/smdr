//! Daemonization support — forks the process into the background so the
//! calling shell is not blocked.
//!
//! On Unix the classic double-fork + `setsid` technique is used:
//! 1. Fork once; the parent exits immediately.
//! 2. The child calls `setsid()` to become a new session leader.
//! 3. Fork again; the intermediate child exits so the grandchild can never
//!    re-acquire a controlling terminal.
//! 4. Redirect stdin/stdout/stderr to `/dev/null`.
//!
//! On non-Unix platforms this is a no-op.
//!
//! The function is `unsafe` because it uses `fork(2)`, which has undefined
//! behaviour if other threads are running.  It must be called **before** any
//! threads (including the tokio runtime) are spawned.

#[cfg(unix)]
pub(crate) fn daemonize() {
    use std::os::fd::RawFd;

    unsafe {
        // First fork
        let pid = libc::fork();
        if pid < 0 {
            eprintln!("smdr: first fork failed — running in foreground");
            return;
        }
        if pid > 0 {
            // Parent exits immediately so the shell returns.
            std::process::exit(0);
        }

        // Child: become session leader.
        if libc::setsid() < 0 {
            eprintln!("smdr: setsid failed — running in foreground");
            return;
        }

        // Second fork (prevents re-acquiring a controlling terminal).
        let pid = libc::fork();
        if pid < 0 {
            eprintln!("smdr: second fork failed — running in foreground");
            return;
        }
        if pid > 0 {
            // Intermediate child exits.
            std::process::exit(0);
        }

        // Grandchild continues as the daemon.

        // Redirect stdin/stdout/stderr to /dev/null.
        let devnull = std::ffi::CString::new("/dev/null").unwrap();
        let fd = libc::open(devnull.as_ptr(), libc::O_RDWR);
        if fd >= 0 {
            libc::dup2(fd, 0 as RawFd);
            libc::dup2(fd, 1 as RawFd);
            libc::dup2(fd, 2 as RawFd);
            if fd > 2 {
                libc::close(fd);
            }
        }

        // Reset umask so files are created with sensible permissions.
        libc::umask(0o022);
    }
}

#[cfg(not(unix))]
pub(crate) fn daemonize() {
    // No-op on non-Unix platforms.
}
