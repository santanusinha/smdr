//! Daemonization support — forks the process into the background so the
//! calling shell is not blocked.
//!
//! ## Platform split
//!
//! **Linux/BSD:** the classic double-fork + `setsid` technique is used:
//! 1. Fork once; the parent exits immediately.
//! 2. The child calls `setsid()` to become a new session leader.
//! 3. Fork again; the intermediate child exits so the grandchild can never
//!    re-acquire a controlling terminal.
//! 4. Redirect stdin/stdout/stderr to `/dev/null`.
//!
//! **macOS:** fork-then-run-GUI crashes with SIGBUS. After `fork(2)` the
//! child must not initialize AppKit/CoreFoundation in-process: frameworks
//! discover their state via Mach/IPC and os_log when a Cocoa subsystem
//! first initializes, and the child-side of a fork cannot safely do that
//! (Apple documents only `exec`- or exit-asap-safe usage after fork in
//! Objective-C/Swift programs). The observed crash is
//! `SIGBUS: KERN_PROTECTION_FAILURE` in `_os_log_preferences_refresh` on
//! the way to `-[NSMenuItem setTitle:]` while iced builds the menu bar.
//!
//! The correct classic remedy is **fork + setsid + `execv` of self**:
//! the forked child immediately replaces its process image with a fresh
//! copy of smdr, so AppKit initializes in a pristine (non-forked) process.
//! An env marker (`SMDR_DAEMONIZED=1`) plus the original argv makes the
//! re-exec'd grandchild skip daemonization again and go straight to the
//! GUI. Any failure at any step (fork, setsid, exec) falls back to running
//! in the foreground — a window in the foreground beats no window at all.
//!
//! On non-Unix platforms this is a no-op.

/// Env var marking a process that was already daemonized (via re-exec) so
/// it does not daemonize again in an infinite loop.
#[cfg(unix)]
const DAEMON_ENV: &str = "SMDR_DAEMONIZED";

#[cfg(unix)]
pub(crate) fn daemonize(original_args: &[String]) -> bool {
    use std::os::fd::RawFd;
    use std::os::unix::ffi::OsStrExt as _;

    // Already re-exec'd once: this process is the daemon. Attach stdio to
    // /dev/null (exec preserves fds, so the fork child's /dev/null wiring
    // carried over) and continue into the GUI.
    if std::env::var_os(DAEMON_ENV).is_some() {
        return false;
    }

    // SAFETY: fork(2) has undefined behaviour if other threads are running.
    // The caller guarantees this runs before any threads are spawned.
    unsafe {
        // First fork.
        let pid = libc::fork();
        if pid < 0 {
            eprintln!("smdr: fork failed — running in foreground");
            return false;
        }
        if pid > 0 {
            // Parent exits immediately so the shell returns.
            std::process::exit(0);
        }

        // Child: become session leader.
        if libc::setsid() < 0 {
            eprintln!("smdr: setsid failed — running in foreground");
            return false;
        }

        // Detach stdio from the terminal before re-exec: /dev/null wiring
        // survives exec, so the re-exec'd daemon starts fully detached.
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

        // macOS: never initialize AppKit in a forked child. Replace the
        // process image with a fresh smdr; the grandchild re-enters main,
        // sees SMDR_DAEMONIZED=1 and skips daemonization.
        #[cfg(target_os = "macos")]
        {
            let exe = match std::env::current_exe() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("smdr: cannot locate own executable: {e} — running in foreground");
                    return false;
                }
            };
            let exe_c = match std::ffi::CString::new(exe.as_os_str().as_bytes()) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("smdr: executable path not valid — running in foreground");
                    return false;
                }
            };
            let mut argv: Vec<std::ffi::CString> = Vec::with_capacity(original_args.len() + 1);
            argv.push(exe_c.clone());
            for a in original_args {
                match std::ffi::CString::new(a.as_str()) {
                    Ok(s) => argv.push(s),
                    Err(_) => {
                        eprintln!("smdr: argument not valid — running in foreground");
                        return false;
                    }
                }
            }
            let mut argv_raw: Vec<*const std::ffi::c_char> =
                argv.iter().map(|s| s.as_ptr()).collect();
            argv_raw.push(std::ptr::null());

            // Marker for the re-exec'd process.
            std::env::set_var(DAEMON_ENV, "1");

            // execv only returns on failure.
            libc::execv(exe_c.as_ptr(), argv_raw.as_ptr());

            // Still here? exec failed. Restore foreground mode: undo the
            // /dev/null wiring so any GUI error output is visible again.
            let term = std::ffi::CString::new("/dev/tty").unwrap();
            let tfd = libc::open(term.as_ptr(), libc::O_RDWR);
            if tfd >= 0 {
                libc::dup2(tfd, 0 as RawFd);
                libc::dup2(tfd, 1 as RawFd);
                libc::dup2(tfd, 2 as RawFd);
                if tfd > 2 {
                    libc::close(tfd);
                }
            }
            eprintln!("smdr: re-exec failed — running in foreground");
            false
        }

        // Linux/BSD: the fork child itself becomes the daemon (in-process
        // daemonization is safe without AppKit).
        #[cfg(not(target_os = "macos"))]
        {
            // Second fork (prevents re-acquiring a controlling terminal).
            let pid = libc::fork();
            if pid < 0 {
                eprintln!("smdr: second fork failed — running in foreground");
                return false;
            }
            if pid > 0 {
                // Intermediate child exits.
                std::process::exit(0);
            }
            // Grandchild continues as the daemon.
            false
        }
    }
}

#[cfg(unix)]
pub(crate) fn is_daemonized() -> bool {
    std::env::var_os(DAEMON_ENV).is_some()
}

#[cfg(not(unix))]
pub(crate) fn daemonize(_original_args: &[String]) -> bool {
    // No-op on non-Unix platforms: run in the foreground.
    false
}

#[cfg(not(unix))]
pub(crate) fn is_daemonized() -> bool {
    false
}
