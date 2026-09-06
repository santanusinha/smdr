# Title
GUI crashes with SIGBUS on macOS 14 (fork-before-AppKit in daemonize)

## Summary

`smdr <file>` on macOS never opens a window. The launcher daemonizes via double-fork (`src/daemon.rs`), and the forked child then initializes AppKit. CoreFoundation state is invalid after `fork(2)`, so the process dies with `SIGBUS: KERN_PROTECTION_FAILURE` inside os_log/CFBundle machinery while iced builds the NSMenu. Because `daemonize()` also redirects stderr to `/dev/null`, the failure is completely silent — the shell prompt returns with exit 0 and no error anywhere.

## Environment

- smdr 0.1.6 (Homebrew `santanusinha/smdr`, arm64 native, not Rosetta)
- macOS 14.8.2 (Apple Silicon, M-series)
- Terminal: iTerm2
- Reproduces every run; 5 identical crash reports today

## Crash signature (from .ips report)

```
Exception Type:  EXC_BAD_ACCESS (SIGBUS)
Termination:     KERN_PROTECTION_FAILURE
ASI:             "crashed on child side of fork pre-exec"

Thread 0 (main):
  _os_log_preferences_refresh
  os_log_type_enabled
  _CFBundleCopyLoadedImagePathForPointer
  _CFBundleGetBundleWithIdentifier
  +[NSBundle bundleWithIdentifier:]
  +[NSImage _systemImageNamed:]
  +[NSImage imageNamed:]
  -[NSMenuItem setTitle:]
  +[NSMenuItem separatorItem]
  <iced menu-bar setup>
  -[NSApplication _sendFinishLaunchingNotification]
```

## Root cause

`main.rs` calls `daemon::daemonize()` (double-fork, stdio→/dev/null) before `render::launch()`. The grandchild then boots iced → winit → AppKit in the forked child process. After `fork(2)` a child may not initialize Cocoa/CoreFoundation subsystems that lazily discover state via Mach IPC and os_log — Apple's own guidance is that after fork, a program that will use such frameworks should `exec` or exit as soon as possible. The GUI is guaranteed to touch them, so the current design is unsafe on macOS by construction.

Secondary effect: stderr is wired to `/dev/null` before `render::launch`, so even recoverable launch errors are invisible.

## Repro

```sh
brew install santanusinha/smdr/smdr
smdr README.md
# prompt returns, no window, exit 0
ls ~/Library/Logs/DiagnosticReports/ | grep smdr
```

Interesting detail: the stdin path (`cat README.md | smdr`) works, because `main.rs` only daemonizes on the file-argument path; `--review` also works because it skips daemonization to keep stdout usable.

## Suggested fix

On macOS, replace "fork and continue in child" with "fork → setsid → `execv(self)`", passing the original argv plus an env marker (e.g. `SMDR_DAEMONIZED=1`) so the re-exec'd process skips daemonization and goes straight to the GUI. AppKit then initializes in a pristine process image. Linux/BSD can keep the existing double-fork. All failure paths (fork/setsid/execv) should fall back to running in the foreground instead of silently dying.

I have a working patch on this exact design (fork+setsid+execv, env marker, foreground fallback; Linux/BSD double-fork untouched). Verified on macOS 14.8.2 arm64: file-arg launch, `-w`, tab hand-off, stdin, `--review`, headless review all pass; full test suite green. Happy to open a PR if useful.
