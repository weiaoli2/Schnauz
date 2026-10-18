//! Visible transfer progress in the fixed footer. Network copies retain SFTP;
//! a private PTY exposes OpenSSH's real per-file meter, never a user's terminal.
use super::*;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Meter {
    pub file: String,
    pub percent: Option<u8>,
}
impl Meter {
    fn parse(line: &str) -> Option<Self> {
        let end = line.rfind('%')?;
        let start = line[..end]
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |i| i + 1);
        let percent = line[start..end].parse::<u8>().ok().filter(|p| *p <= 100)?;
        let rest: Vec<_> = line[end + 1..].split_whitespace().collect();
        if rest.len() < 2
            || !rest[0].starts_with(|c: char| c.is_ascii_digit())
            || !rest.iter().any(|s| s.ends_with("/s"))
        {
            return None;
        }
        Some(Self {
            file: line[..start].trim().to_string(),
            percent: Some(percent),
        })
    }
}
struct Batch(PathBuf);
impl Batch {
    fn new(script: &str) -> Result<Self, String> {
        use std::os::unix::fs::OpenOptionsExt;
        static NEXT: AtomicU64 = AtomicU64::new(1);
        for _ in 0..10 {
            let nonce = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "schnauz-transfer-{}-{}-{nonce:x}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(mut file) => {
                    let batch = Self(path);
                    file.write_all(script.as_bytes())
                        .and_then(|_| file.write_all(b"\nquit\n"))
                        .map_err(|e| e.to_string())?;
                    return Ok(batch);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Couldn't prepare transfer.".into())
    }
}
impl Drop for Batch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) fn sftp(
    server: &SftpServer,
    script: &str,
    use_system: bool,
    progress: &Arc<JobProgress>,
) -> Result<(), String> {
    let server = ssh_connection::for_reconnect(server, use_system);
    ensure_master(&server)?;
    let mut command = CommandBuilder::new("/usr/bin/sftp");
    command.args(ssh_options(&server, use_system));
    if server.port != 0 {
        command.args(["-P", &server.port.to_string()]);
    }
    run(command, &server.connection_target(), script, progress)
}
fn run(
    mut command: CommandBuilder,
    target: &str,
    script: &str,
    progress: &Arc<JobProgress>,
) -> Result<(), String> {
    if progress.cancel.load(Ordering::Relaxed) {
        return Err("Copy cancelled".into());
    }
    // Batch mode disables the meter independently of quiet mode. Enable it
    // explicitly, using the same batch mode as existing transfers.
    let batch = Batch::new(&format!("progress\n{script}"))?;
    command.arg("-b");
    command.arg(&batch.0);
    command.arg(target);
    command.env("LC_ALL", "C");
    command.env("TERM", "dumb");
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 160,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| e.to_string())?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    let captured = output.clone();
    let meter = progress.clone();
    let reader = std::thread::spawn(move || {
        let mut bytes = [0; 4096];
        let mut line = Vec::new();
        while let Ok(n) = reader.read(&mut bytes) {
            if n == 0 {
                break;
            }
            {
                let mut tail = captured.lock().unwrap();
                tail.extend_from_slice(&bytes[..n]);
                if tail.len() > 16384 {
                    let extra = tail.len() - 16384;
                    tail.drain(..extra);
                }
            }
            for b in &bytes[..n] {
                if matches!(*b, b'\r' | b'\n') {
                    if let Some(value) = Meter::parse(&String::from_utf8_lossy(&line)) {
                        *meter.network.lock().unwrap() = Some(value);
                    }
                    line.clear();
                } else if line.len() < 8192 {
                    line.push(*b);
                }
            }
        }
    });
    let status = loop {
        if progress.cancel.load(Ordering::Relaxed) {
            // Only this newly owned PTY process group, never a master/user shell.
            if let Some(pid) = child.process_id().filter(|pid| *pid > 1) {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGTERM);
                }
            }
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(80)),
            Err(e) => {
                let _ = child.kill();
                break Err(e.to_string());
            }
        }
    };
    let _ = reader.join();
    drop(pair.master);
    if progress.cancel.load(Ordering::Relaxed) {
        return Err("Copy cancelled · Partial files may remain at the destination".into());
    }
    if status?.success() {
        Ok(())
    } else {
        let bytes = output.lock().unwrap();
        Err(ssh_connection::error_message(&String::from_utf8_lossy(
            &bytes,
        )))
    }
}

impl Schnauz {
    pub(super) fn begin_file_job(
        &mut self,
        label: String,
        prog: Arc<JobProgress>,
        cx: &mut Context<Self>,
    ) -> u64 {
        self.next_job_id += 1;
        let id = self.next_job_id;
        let was_idle = self.jobs.is_empty();
        self.jobs.push(FileJob { id, label, prog });
        cx.notify();
        if was_idle {
            cx.spawn(async move |this, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if !matches!(
                    this.update(cx, |this, cx| {
                        cx.notify();
                        !this.jobs.is_empty()
                    }),
                    Ok(true)
                ) {
                    break;
                }
            })
            .detach();
        }
        id
    }
    pub(super) fn render_transfer_status(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let job = self.jobs.first()?;
        let total = job.prog.total.load(Ordering::Relaxed);
        let done = job.prog.done.load(Ordering::Relaxed).min(total);
        let network = job.prog.network.lock().unwrap().clone();
        let percent = network
            .as_ref()
            .and_then(|m| m.percent)
            .or_else(|| (total > 0).then(|| ((done as u128 * 100) / total as u128) as u8));
        let caption = match &network {
            Some(m) if !m.file.is_empty() => format!("{} · Current file: {}", job.label, m.file),
            _ => job.label.clone(),
        };
        let detail = if let Some(m) = &network {
            if m.percent.is_some() {
                "Current file".to_string()
            } else {
                "Preparing…".into()
            }
        } else if total > 0 {
            format!(
                "{} of {}",
                format_size(false, done),
                format_size(false, total)
            )
        } else {
            "Preparing…".into()
        };
        let current = network.as_ref().filter(|m| !m.file.is_empty()).map_or_else(
            || job.label.clone(),
            |m| format!("Copying {} · current file", m.file),
        );
        let label = if self.jobs.len() > 1 {
            format!("{current} · +{}", self.jobs.len() - 1)
        } else {
            current
        };
        let prog = job.prog.clone();
        let cancelled = prog.cancel.load(Ordering::Relaxed);
        let id = job.id;
        Some(
            div()
                .id("footer-transfer")
                .flex()
                .items_center()
                .min_w_0()
                .max_w_full()
                .h(px(23.))
                .px_2()
                .gap_2()
                .rounded_md()
                .bg(rgb(0x3478c5))
                .text_color(rgb(0xffffff))
                .tooltip(tip(format!("{caption}\n{detail}")))
                .child(div().min_w_0().truncate().child(if cancelled {
                    "Cancelling…".into()
                } else {
                    label
                }))
                .child(
                    div()
                        .flex_none()
                        .w(px(74.))
                        .h(px(4.))
                        .rounded_full()
                        .bg(rgba(0xffffff44))
                        .child(
                            div()
                                .h_full()
                                .w(relative(percent.unwrap_or(0) as f32 / 100.))
                                .rounded_full()
                                .bg(rgb(0xffffff)),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(31.))
                        .child(percent.map_or_else(|| "…".into(), |p| format!("{p}%"))),
                )
                .child(
                    div()
                        .id(("footer-transfer-cancel", id as usize))
                        .flex_none()
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .rounded_sm()
                        .hover(|s| s.bg(rgba(0xffffff33)))
                        .child("×")
                        .tooltip(tip("Cancel transfer"))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            prog.cancel.store(true, Ordering::Relaxed);
                            cx.notify();
                        })),
                )
                .children(ui_checks::probe("footer-transfer"))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percent_is_real_per_file_progress_not_a_number_in_a_filename() {
        assert_eq!(
            Meter::parse("file with 50% in its name.txt       42%  123KB  1.2MB/s   00:01 ETA"),
            Some(Meter {
                file: "file with 50% in its name.txt".into(),
                percent: Some(42)
            })
        );
        assert_eq!(
            Meter::parse("file.txt 100% 1KB 20.0KB/s 00:00")
                .unwrap()
                .percent,
            Some(100)
        );
        assert!(Meter::parse("sftp> put 25% report.txt").is_none());
        assert!(Meter::parse("file 101% 1KB 2KB/s 00:00").is_none());
    }
    #[test]
    fn cancellation_stops_only_the_new_transfer_and_removes_its_private_batch() {
        let progress = Arc::new(JobProgress::default());
        let cancel = progress.clone();
        let signal = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            cancel.cancel.store(true, Ordering::Relaxed);
        });
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "sleep 20", "owned-transfer-fixture"]);
        let started = Instant::now();
        assert!(run(command, "fixture", "", &progress)
            .unwrap_err()
            .starts_with("Copy cancelled"));
        signal.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
        let batch = Batch::new("fixture").unwrap();
        let path = batch.0.clone();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(batch);
        assert!(!path.exists());
    }
    #[test]
    fn real_sftp_pty_copies_quoted_paths_and_reports_progress_without_ssh() {
        let root = crate::tests::scratch("transfer-meter");
        let source = root.join("a \"quoted\" [x].txt");
        let dest = root.join("copy.txt");
        fs::write(&source, vec![b'x'; 1024 * 1024]).unwrap();
        let mut command = CommandBuilder::new("/usr/bin/sftp");
        command.args(["-D", "/usr/libexec/sftp-server"]);
        let script = format!(
            "put {} {}",
            sftp_path_arg(source.to_str().unwrap()).unwrap(),
            sftp_path_arg(dest.to_str().unwrap()).unwrap()
        );
        let progress = Arc::new(JobProgress::default());
        *progress.network.lock().unwrap() = Some(Meter::default());
        run(command, "fixture", &script, &progress).unwrap();
        assert_eq!(fs::read(&source).unwrap(), fs::read(&dest).unwrap());
        let folder = root.join("Folder with spaces");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("nested.txt"), "recursive transfer").unwrap();
        let downloaded = root.join("downloaded");
        let mut download = CommandBuilder::new("/usr/bin/sftp");
        download.args(["-D", "/usr/libexec/sftp-server"]);
        let script = format!(
            "get -R {} {}",
            sftp_path_arg(folder.to_str().unwrap()).unwrap(),
            sftp_path_arg(downloaded.to_str().unwrap()).unwrap()
        );
        run(download, "fixture", &script, &progress).unwrap();
        assert_eq!(
            fs::read_to_string(downloaded.join("nested.txt")).unwrap(),
            "recursive transfer"
        );
        assert_eq!(
            progress.network.lock().unwrap().as_ref().unwrap().percent,
            Some(100)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
