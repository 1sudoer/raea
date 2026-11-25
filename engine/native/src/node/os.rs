// What node.js's fs and child_process report that Unix and Windows keep differently.

/// fs.stat's mode and ctimeMs (the inode change time).
#[cfg(unix)]
pub(super) fn mode_ctime(m: &std::fs::Metadata) -> (u32, f64) {
    use std::os::unix::fs::MetadataExt;
    (m.mode(), m.ctime() as f64 * 1e3 + m.ctime_nsec() as f64 / 1e6)
}

/// Windows has no mode bits or ctime: Node's own stat reports a directory as 0o40666 and a file as 0o100666 (0o444
/// read-only), and ctimeMs as the last write.
#[cfg(windows)]
pub(super) fn mode_ctime(m: &std::fs::Metadata) -> (u32, f64) {
    let perm = if m.permissions().readonly() { 0o444 } else { 0o666 };
    ((if m.is_dir() { 0o40000 } else { 0o100000 }) | perm,super::millis(m.modified()))
}

/// The signal that ended a child, as Node names it (null when it exited with a code).
#[cfg(unix)]
pub(super) fn signal(st: &std::process::ExitStatus) -> Option<String> {
    std::os::unix::process::ExitStatusExt::signal(st).map(signal_name)
}

/// Windows has no signals: a child always exits with a code.
#[cfg(windows)]
pub(super) fn signal(_: &std::process::ExitStatus) -> Option<String> {
    None
}

#[cfg(unix)]
fn signal_name(n: i32) -> String {
    match n {
        libc::SIGTERM => "SIGTERM".into(),
        libc::SIGKILL => "SIGKILL".into(),
        libc::SIGINT => "SIGINT".into(),
        libc::SIGSEGV => "SIGSEGV".into(),
        libc::SIGABRT => "SIGABRT".into(),
        n => format!("SIG{n}"),
    }
}

/// process.arch: Node's word for the CPU this was built for
pub(super) fn node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "ia32",
        other => other,
    }
}

/// process.ppid: the process that started this one. Windows keeps it in a process's own record, read here through
/// PowerShell (once, when asked: the agent shell asks only to find the harness that ran `raea show`).
pub(super) fn parent_pid() -> Option<u32> {
    #[cfg(unix)]
    {
        // SAFETY: getppid takes nothing and can't fail
        Some(unsafe { libc::getppid() } as u32)
    }
    #[cfg(windows)]
    {
        let q = format!("(Get-CimInstance Win32_Process -Filter 'ProcessId={}').ParentProcessId", std::process::id());
        let o = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &q]).stdin(std::process::Stdio::null()).output().ok()?;
        String::from_utf8_lossy(&o.stdout).trim().parse().ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}
