//! Owned background work and bounded subprocess waits. Never controls a live PTY.
use std::{
    io::{Read, Write},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(super) async fn run<T: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let _ = tx.send(objc2::rc::autoreleasepool(|_| work()));
        })
        .map_err(|_| "Could not start background work.".to_string())?;
    rx.await
        .map_err(|_| "Background work stopped unexpectedly.".to_string())
}

pub(super) async fn result<T: Send + 'static>(
    name: &str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    run(name, work).await.and_then(|r| r)
}

fn capture(mut pipe: impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = pipe.read(&mut buffer)?;
        if n == 0 {
            return Ok(result);
        }
        if result.len().saturating_add(n) > limit {
            return Err(std::io::Error::other(
                "Command output exceeded its size limit.",
            ));
        }
        result.extend_from_slice(&buffer[..n]);
    }
}

/// Kills only the newly spawned child on cancellation/deadline. Existing SSH
/// masters, shells and process groups are never signalled.
pub(super) fn command(
    cmd: Command,
    input: Vec<u8>,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
) -> Result<Output, String> {
    command_guarded(cmd, input, timeout, cancel, None)
}

pub(super) fn command_guarded(
    mut cmd: Command,
    input: Vec<u8>,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    file_limit: Option<(std::path::PathBuf, u64)>,
) -> Result<Output, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Operation cancelled.".into());
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start command: {e}"))?;
    let stdin = child.stdin.take().ok_or("Command input unavailable")?;
    let stdout = child.stdout.take().ok_or("Command output unavailable")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Command error output unavailable")?;
    let (tx, rx) = std::sync::mpsc::channel();
    let out = tx.clone();
    let err = tx.clone();
    let spawn = |name: &str, work: Box<dyn FnOnce() + Send>| {
        std::thread::Builder::new().name(name.into()).spawn(work)
    };
    let writers = spawn(
        "schnauz-command-input",
        Box::new(move || {
            let mut stdin = stdin;
            let _ = stdin.write_all(&input);
        }),
    );
    let readers = spawn(
        "schnauz-command-output",
        Box::new(move || {
            let _ = out.send((false, capture(stdout, 32 * 1024 * 1024)));
        }),
    );
    let errors = spawn(
        "schnauz-command-errors",
        Box::new(move || {
            let _ = err.send((true, capture(stderr, 256 * 1024)));
        }),
    );
    drop(tx);
    if writers.is_err() || readers.is_err() || errors.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Could not start command readers.".into());
    }
    let deadline = Instant::now() + timeout;
    let mut captured = [None, None];
    let mut status = None;
    loop {
        let error = if cancel.load(Ordering::Acquire) {
            Some("Operation cancelled.".to_string())
        } else if file_limit
            .as_ref()
            .is_some_and(|(path, limit)| std::fs::metadata(path).is_ok_and(|m| m.len() > *limit))
        {
            Some("Preview exceeds the download size limit.".into())
        } else if Instant::now() >= deadline {
            Some("Operation timed out.".into())
        } else {
            None
        };
        if let Some(error) = error {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        while let Ok((is_error, result)) = rx.try_recv() {
            match result {
                Ok(bytes) => captured[usize::from(is_error)] = Some(bytes),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e.to_string());
                }
            }
        }
        if status.is_none() {
            status = child.try_wait().map_err(|e| {
                let _ = child.kill();
                let _ = child.wait();
                e.to_string()
            })?;
        }
        if let Some(status) = status {
            if captured.iter().all(Option::is_some) {
                return Ok(Output {
                    status,
                    stdout: captured[0].take().unwrap(),
                    stderr: captured[1].take().unwrap(),
                });
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadline_reaps_only_its_own_child() {
        let mut unrelated = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let start = Instant::now();
        let result = super::command(
            command,
            Vec::new(),
            Duration::from_millis(40),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(result.is_err() && start.elapsed() < Duration::from_secs(2));
        assert!(unrelated.try_wait().unwrap().is_none());
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }
}
