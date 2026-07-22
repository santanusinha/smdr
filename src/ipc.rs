//! Single-instance enforcement and IPC — passes file paths to an existing
//! smdr instance over a Unix domain socket so that additional documents open
//! as **tabs in the running window** instead of spawning new windows.
//!
//! Flow when smdr starts with a file argument:
//! 1. [`client_send`] tries to connect to the socket and hand off the path.
//!    * On success → an instance is already running; the caller exits.
//!    * On failure (no server / stale socket) → the caller becomes the first
//!      instance, daemonizes, and launches the GUI.
//! 2. The first instance drives [`server_worker`] as an iced subscription,
//!    accepting incoming paths and surfacing them to the UI (which opens a
//!    new tab).
//!
//! The socket path is `$XDG_RUNTIME_DIR/smdr-<uid>.sock`, falling back to
//! `/tmp/smdr-<uid>.sock`.

use std::path::PathBuf;

/// Returns the IPC socket path for the current user.
pub fn socket_path() -> Option<PathBuf> {
    // XDG_RUNTIME_DIR is preferred (per-user, tmpfs, auto-cleaned).
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let uid = uid();
        return Some(PathBuf::from(dir).join(format!("smdr-{uid}.sock")));
    }

    // Fall back to /tmp.
    let uid = uid();
    Some(PathBuf::from(format!("/tmp/smdr-{uid}.sock")))
}

/// Returns the current process's real UID via `getuid(2)`.
#[cfg(unix)]
fn uid() -> u32 {
    // SAFETY: `getuid` has no safety requirements.
    unsafe { libc::getuid() }
}

#[cfg(not(unix))]
fn uid() -> u32 {
    0
}

// ---------------------------------------------------------------------------
// Client side: hand a path to an already-running instance
// ---------------------------------------------------------------------------

/// Try to hand `file_paths` to an already-running smdr instance.
///
/// All paths are sent over a single connection, newline-separated, so a batch
/// of files passed on one command line each open as their own tab.
///
/// This uses a **blocking** `std` Unix socket rather than tokio so it is safe
/// to call before [`crate::daemon::daemonize`] forks the process — spinning up
/// a tokio runtime before `fork(2)` would be undefined behaviour.
///
/// # Errors
/// Returns `Err` if no instance is listening (connection refused / socket
/// missing) or the write fails.  The caller should then become the server.
#[cfg(unix)]
pub fn client_send(file_paths: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let path = socket_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no socket path"))?;

    let mut stream = UnixStream::connect(&path)?;
    for file_path in file_paths {
        stream.write_all(file_path.as_bytes())?;
        stream.write_all(b"\n")?;
    }
    stream.flush()?;
    Ok(())
}

#[cfg(not(unix))]
pub fn client_send(_file_paths: &[String]) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "IPC single-instance not supported on this platform",
    ))
}

// ---------------------------------------------------------------------------
// Server side: run inside the iced runtime on the first instance
// ---------------------------------------------------------------------------

/// A `Stream` that binds the IPC socket and yields each file path received
/// from later smdr invocations.
///
/// Intended to be driven by [`iced::Subscription::run`]; the runtime's tokio
/// executor powers the async accept loop.  Because only the first instance
/// launches the GUI (later invocations exit via [`client_send`]), the socket
/// is bound exactly once.
#[cfg(unix)]
pub fn server_worker() -> impl iced::futures::Stream<Item = PathBuf> {
    iced::stream::channel(
        16,
        |mut output: iced::futures::channel::mpsc::Sender<PathBuf>| async move {
            use iced::futures::SinkExt;
            use tokio::io::AsyncReadExt;
            use tokio::net::UnixListener;

            let Some(path) = socket_path() else {
                return;
            };

            // Remove a stale socket file left over from a previous run.
            let _ = std::fs::remove_file(&path);

            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("smdr: could not bind IPC socket: {e}");
                    return;
                }
            };

            // Restrict the socket to the owner (0600).
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                let _ = std::fs::set_permissions(&path, perms);
            }

            loop {
                match listener.accept().await {
                    Ok((mut stream, _)) => {
                        // Read the whole request (a single newline-terminated path).
                        let mut buf = String::new();
                        if stream.read_to_string(&mut buf).await.is_ok() {
                            for line in buf.lines() {
                                let line = line.trim();
                                if !line.is_empty() {
                                    let _ = output.send(PathBuf::from(line)).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("smdr: IPC accept error: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        },
    )
}

#[cfg(not(unix))]
pub fn server_worker() -> impl iced::futures::Stream<Item = PathBuf> {
    iced::futures::stream::pending()
}

/// Remove the IPC socket file (best-effort) on shutdown.
pub fn cleanup_socket() {
    if let Some(path) = socket_path() {
        let _ = std::fs::remove_file(&path);
    }
}
