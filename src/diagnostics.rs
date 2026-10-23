//! Local, bounded crash evidence. Never records terminal contents or credentials.
use std::{
    backtrace::Backtrace,
    fs,
    io::Write,
    os::fd::IntoRawFd,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static FD: AtomicI32 = AtomicI32::new(-1);
static WRITTEN: AtomicUsize = AtomicUsize::new(0);
static FATAL_WRITTEN: AtomicUsize = AtomicUsize::new(0);
const LIMIT: usize = 256 * 1024;
const KEEP: usize = 12;

pub(super) fn install() {
    let directory = if let Some(root) =
        std::env::var_os("SCHNAUZ_CONFIG_DIR").or_else(crate::legacy::config_override)
    {
        PathBuf::from(root).join("diagnostics")
    } else {
        crate::home_dir().join("Library/Logs/Schnauz")
    };
    if let Err(error) = install_at(&directory) {
        eprintln!("Could not enable Schnauz crash diagnostics: {error}");
    }
}

fn install_at(directory: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = directory.join(format!("session-{timestamp}-{}.log", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    writeln!(file, "Schnauz {} | {} | {} | unix_ms={timestamp} | pid={}\nLocal diagnostics. Session start does not indicate a crash.\nNo terminal output, commands, file contents or credentials are collected.", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH, std::process::id())?;
    FD.store(file.into_raw_fd(), Ordering::Relaxed);
    prune(directory);
    std::panic::set_hook(Box::new(|error| {
        // Bypass the normal event budget and capture even without RUST_BACKTRACE.
        // Write directly: aborting a native callback cannot wait for a log worker.
        fatal(&format!(
            "\nRUST PANIC\n{error}\nThread: {:?}\nStack trace:\n{}\n",
            std::thread::current().name(),
            Backtrace::force_capture()
        ));
    }));
    unsafe {
        NSSetUncaughtExceptionHandler(Some(native_exception));
        for signal in [
            libc::SIGABRT,
            libc::SIGSEGV,
            libc::SIGBUS,
            libc::SIGILL,
            libc::SIGFPE,
        ] {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = fatal_signal as *const () as usize;
            action.sa_flags = libc::SA_RESETHAND;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
    }
    Ok(())
}

fn prune(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut reports: Vec<_> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("session-")
                && name.ends_with(".log")
                && e.file_type().is_ok_and(|t| t.is_file())
        })
        .map(|e| e.path())
        .collect();
    reports.sort();
    for report in reports.iter().take(reports.len().saturating_sub(KEEP)) {
        let _ = fs::remove_file(report);
    }
}

/// Only fixed labels belong here. Never pass user paths, commands or content.
pub(super) fn event(label: &'static str) {
    if WRITTEN.fetch_add(label.len() + 64, Ordering::Relaxed) < LIMIT / 2 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        raw(format!("unix_ms={timestamp} event={label}\n").as_bytes());
    }
}
fn fatal(report: &str) {
    let used = FATAL_WRITTEN.fetch_add(report.len().min(LIMIT / 2), Ordering::Relaxed);
    let mut end = report.len().min((LIMIT / 2).saturating_sub(used));
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    raw(report[..end].as_bytes());
    unsafe {
        libc::fsync(FD.load(Ordering::Relaxed));
    }
}
fn raw(bytes: &[u8]) {
    let fd = FD.load(Ordering::Relaxed);
    if fd < 0 {
        return;
    }
    let mut offset = 0;
    while offset < bytes.len() {
        let count =
            unsafe { libc::write(fd, bytes[offset..].as_ptr().cast(), bytes.len() - offset) };
        if count <= 0 {
            break;
        }
        offset += count as usize;
    }
}

extern "C" fn fatal_signal(signal: libc::c_int) {
    // Async-signal-safe operations only; keep native macOS crash handling intact.
    let message: &[u8] = match signal {
        libc::SIGABRT => b"\nFATAL SIGNAL: SIGABRT\n",
        libc::SIGSEGV => b"\nFATAL SIGNAL: SIGSEGV\n",
        libc::SIGBUS => b"\nFATAL SIGNAL: SIGBUS\n",
        libc::SIGILL => b"\nFATAL SIGNAL: SIGILL\n",
        _ => b"\nFATAL SIGNAL: SIGFPE\n",
    };
    raw(message);
    unsafe {
        libc::raise(signal);
    }
}

#[link(name = "Foundation", kind = "framework")]
extern "C" {
    fn NSSetUncaughtExceptionHandler(
        handler: Option<unsafe extern "C" fn(*mut objc2::runtime::AnyObject)>,
    );
}
unsafe extern "C" fn native_exception(exception: *mut objc2::runtime::AnyObject) {
    if exception.is_null() {
        return;
    }
    let name: *mut objc2_foundation::NSString = objc2::msg_send![exception, name];
    let reason: *mut objc2_foundation::NSString = objc2::msg_send![exception, reason];
    let stack: *mut objc2::runtime::AnyObject = objc2::msg_send![exception, callStackSymbols];
    let joined: *mut objc2_foundation::NSString = if stack.is_null() {
        std::ptr::null_mut()
    } else {
        objc2::msg_send![stack, componentsJoinedByString: &*objc2_foundation::NSString::from_str("\n")]
    };
    let string = |ptr: *mut objc2_foundation::NSString| {
        ptr.as_ref().map(|s| s.to_string()).unwrap_or_default()
    };
    fatal(&format!(
        "\nUNCAUGHT NATIVE EXCEPTION\n{}: {}\nStack trace:\n{}\n",
        string(name),
        string(reason),
        if joined.is_null() {
            Backtrace::force_capture().to_string()
        } else {
            string(joined)
        }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crash_child() {
        let Some(dir) = std::env::var_os("SCHNAUZ_DIAGNOSTIC_TEST") else {
            return;
        };
        install_at(Path::new(&dir)).unwrap();
        event("terminal divider drag started");
        unsafe {
            let exception: *mut objc2::runtime::AnyObject = objc2::msg_send![objc2::class!(NSException),
                exceptionWithName: &*objc2_foundation::NSString::from_str("FixtureException"),
                reason: &*objc2_foundation::NSString::from_str("native diagnostic fixture"),
                userInfo: std::ptr::null::<objc2::runtime::AnyObject>()];
            native_exception(exception);
        }
        panic!("diagnostic fixture panic");
    }
    #[test]
    fn panic_report_survives_child_exit_and_retention_preserves_other_files() {
        let dir = crate::tests::scratch("crash-diagnostics");
        for n in 0..16 {
            fs::write(dir.join(format!("session-000{n:02}.log")), "old").unwrap();
        }
        fs::write(dir.join("unrelated.log"), "preserve").unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "diagnostics::tests::crash_child", "--nocapture"])
            .env("SCHNAUZ_DIAGNOSTIC_TEST", &dir)
            .output()
            .unwrap();
        assert!(!child.status.success());
        let reports: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("session-"))
            .collect();
        assert_eq!(reports.len(), KEEP);
        let report = reports
            .iter()
            .map(|e| fs::read_to_string(e.path()).unwrap())
            .find(|s| s.contains("RUST PANIC"))
            .unwrap();
        for text in [
            "Schnauz",
            "terminal divider drag started",
            "diagnostic fixture panic",
            "Stack trace:",
            "diagnostics::tests::crash_child",
            "UNCAUGHT NATIVE EXCEPTION",
            "native diagnostic fixture",
        ] {
            assert!(report.contains(text), "Missing {text}");
        }
        assert_eq!(
            fs::read_to_string(dir.join("unrelated.log")).unwrap(),
            "preserve"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
