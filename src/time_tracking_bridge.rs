//! Optional Codex hooks exchange status only; prompts/output are never retained.
use super::*;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use time_tracking::{Event, Status};

const EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreModelRequest",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
    "Interrupt",
    "SessionEnd",
];

pub(super) fn channel(request: &Path) -> PathBuf {
    static RUN: OnceLock<u128> = OnceLock::new();
    let run = RUN.get_or_init(|| {
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    });
    // A crashed app's channel must never be reused after macOS recycles its PID.
    request.with_extension(format!("time-{run}"))
}

pub(super) fn prepare(
    request: &Path,
    command: &mut portable_pty::CommandBuilder,
) -> Result<(), String> {
    let path = channel(request);
    fs::create_dir(&path).map_err(|e| format!("Couldn't prepare terminal status channel: {e}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    command.env("SCHNAUZ_TIME_CHANNEL", &path);
    command.env("SCHNAUZ_CODEX_STATUS_CONFIG", session_config(&path)?);
    Ok(())
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn status(value: &serde_json::Value) -> Option<Status> {
    // Subagents share the parent's session id. Their intermediate events must
    // not resume a parent that is waiting for the user.
    if value.get("agent_id").is_some_and(|id| !id.is_null()) {
        return None;
    }
    let name = value.get("hook_event_name")?.as_str()?;
    match name {
        "UserPromptSubmit" | "PreModelRequest" | "PostToolUse" => Some(Status::Working),
        "PreToolUse" => {
            let tool = value
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            Some(
                if tool.contains("request_user_input") || tool.contains("askuserquestion") {
                    Status::Waiting
                } else {
                    Status::Working
                },
            )
        }
        "PermissionRequest" => Some(Status::Waiting),
        "SessionStart" | "Stop" | "Interrupt" | "SessionEnd" => Some(Status::Idle),
        _ => None,
    }
}

/// Hook entry point runs before GPUI startup, silently and without networking.
/// Always succeed so disconnected tracking cannot affect Codex execution.
pub(super) fn hook() {
    let _ = hook_inner();
    println!("{{}}");
}
fn hook_inner() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let Some(path) = args.get(2).map(PathBuf::from) else {
        return Ok(());
    };
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Ok(());
    }
    let mut input = Vec::new();
    std::io::stdin().take(1_048_577).read_to_end(&mut input)?;
    if input.len() > 1_048_576 {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_slice(&input)?;
    let Some(status) = status(&value) else {
        return Ok(());
    };
    let Some(session) = value
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| s.len() <= 128)
    else {
        return Ok(());
    };
    let turn = value.get("turn_id").and_then(|v| v.as_str()).unwrap_or("");
    if turn.len() > 128 {
        return Ok(());
    }
    let event = Event {
        session: session.into(),
        turn: turn.into(),
        status,
        at_ms: now_ms(),
        starts_turn: value.get("hook_event_name").and_then(|v| v.as_str())
            == Some("UserPromptSubmit"),
    };
    // A separate atomic file per event keeps concurrent hooks from interleaving.
    let stamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let name = format!("{stamp}-{}.json", std::process::id());
    let temp = path.join(format!(".{name}"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&serde_json::to_vec(&event)?)?;
    drop(file);
    fs::rename(temp, path.join(name))?;
    Ok(())
}

#[derive(Clone)]
pub(super) struct Binding {
    pub channel: PathBuf,
    pub project: String,
    pub running: bool,
    pub enabled: bool,
}

pub(super) fn collect(bindings: Vec<Binding>) -> Vec<(Binding, Vec<Event>)> {
    bindings
        .into_iter()
        .map(|binding| {
            let mut events = Vec::new();
            // Only the shell owned by this app reads this opt-in marker. No command
            // is injected into a live PTY and ordinary untracked Codex is unchanged.
            if binding.channel.is_dir() {
                let marker = binding.channel.join("enabled");
                if binding.enabled && !marker.exists() {
                    let _ = fs::write(marker, b"1");
                } else if !binding.enabled {
                    let _ = fs::remove_file(marker);
                }
            }
            if let Ok(entries) = fs::read_dir(&binding.channel) {
                for entry in entries.take(4096).flatten() {
                    let path = entry.path();
                    if path.extension().is_none_or(|e| e != "json")
                        || path
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                    {
                        continue;
                    }
                    if let Ok(file) = fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NOFOLLOW)
                        .open(&path)
                    {
                        let mut bytes = Vec::new();
                        if file.take(4097).read_to_end(&mut bytes).is_ok() && bytes.len() <= 4096 {
                            if let Ok(event) = serde_json::from_slice::<Event>(&bytes) {
                                events.push(event);
                            }
                        }
                    }
                    let _ = fs::remove_file(path);
                }
            }
            events.sort_by_key(|e| e.at_ms);
            (binding, events)
        })
        .collect()
}

fn session_config(channel: &Path) -> Result<String, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = format!(
        "{} --time-tracking-hook {}",
        shell_quote(&executable.to_string_lossy()),
        shell_quote(&channel.to_string_lossy())
    );
    let quoted = serde_json::to_string(&command).map_err(|e| e.to_string())?;
    let groups = EVENTS
        .iter()
        .map(|event| {
            format!("{event}=[{{hooks=[{{type=\"command\",command={quoted},timeout=3}}]}}]")
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("hooks={{{groups}}}"))
}

pub(super) fn instructions() -> String {
    "In a new Schnauz terminal, start Codex normally (or use schnauz-codex if you have a custom codex alias/function). Open /hooks in Codex and review/trust this terminal's Schnauz status hooks. Start a new turn to connect. Existing sessions and your Codex configuration are unchanged. Requires a Codex version with lifecycle hooks; SSH-hosted Codex is not connected by this local bridge.".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_is_metadata_only_and_subagents_do_not_override_parent() {
        assert_eq!(
            status(&serde_json::json!({"hook_event_name":"PermissionRequest"})),
            Some(Status::Waiting)
        );
        assert_eq!(
            status(&serde_json::json!({"hook_event_name":"PreModelRequest","agent_id":"child"})),
            None
        );
    }
}
