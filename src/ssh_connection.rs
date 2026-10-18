//! Reconnect saved terminal favourites through their original OpenSSH profile.
//! Resolution runs only in the existing background SFTP/SSH workers. No keys,
//! passwords, agent state or live connections are read for profile discovery.
use super::*;

pub(super) fn valid_alias(target: &str) -> Option<&str> {
    let alias = target.rsplit('@').next()?;
    (!alias.is_empty()
        && !alias.starts_with('-')
        && !alias
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "*?!/\\".contains(c)))
    .then_some(alias)
}

#[derive(Clone, Debug)]
struct Profile {
    alias: String,
    host: String,
    user: String,
    port: u16,
}
fn profile(alias: &str, text: &str) -> Option<Profile> {
    let get = |key: &str| text.lines().find_map(|line| line.strip_prefix(key));
    Some(Profile {
        alias: alias.into(),
        host: get("hostname ")?.into(),
        user: get("user ")?.into(),
        port: get("port ")?.parse().ok()?,
    })
}
fn matching_alias<'a>(server: &SftpServer, profiles: &'a [Profile]) -> Option<&'a str> {
    let mut matches = profiles.iter().filter(|p| {
        p.host.eq_ignore_ascii_case(&server.host)
            && p.user == server.user
            && p.port == if server.port == 0 { 22 } else { server.port }
    });
    let first = matches.next()?;
    // Never guess between different authentication/jump-host profiles.
    matches.next().is_none().then_some(first.alias.as_str())
}

// Parse only Host/Include words. Respect quoted paths and comments, never execute
// config text or treat ProxyCommand/Match directives as discovery instructions.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escape = false;
    for ch in line.chars() {
        if escape {
            word.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                word.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '#' => break,
            '=' | ' ' | '\t' => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            _ => word.push(ch),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}
fn aliases_in(path: &Path, home: &Path, seen: &mut HashSet<PathBuf>, out: &mut Vec<String>) {
    if seen.len() >= 32 || out.len() >= 128 || !seen.insert(path.to_path_buf()) {
        return;
    }
    use std::io::Read;
    let mut text = String::new();
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    if file.take(262_145).read_to_string(&mut text).is_err() || text.len() > 262_144 {
        return;
    }
    for line in text.lines() {
        let words = words(line);
        let Some(key) = words.first() else { continue };
        if key.eq_ignore_ascii_case("host") {
            for word in words.iter().skip(1) {
                if valid_alias(word).is_some() && !out.contains(word) && out.len() < 128 {
                    out.push(word.clone());
                }
            }
        } else if key.eq_ignore_ascii_case("include") {
            for word in words.iter().skip(1) {
                let include = if let Some(relative) = word.strip_prefix("~/") {
                    home.join(relative)
                } else if Path::new(word).is_absolute() {
                    PathBuf::from(word)
                } else {
                    home.join(".ssh").join(word)
                };
                // OpenSSH includes support globs. libc performs filename matching
                // only; no shell or command substitution is involved.
                let Ok(pattern) = std::ffi::CString::new(include.as_os_str().as_encoded_bytes())
                else {
                    continue;
                };
                unsafe {
                    let mut glob: libc::glob_t = std::mem::zeroed();
                    if libc::glob(pattern.as_ptr(), 0, None, &mut glob) == 0 {
                        for i in 0..glob.gl_pathc.min(32) {
                            use std::os::unix::ffi::OsStrExt;
                            let name = std::ffi::CStr::from_ptr(*glob.gl_pathv.add(i));
                            aliases_in(
                                Path::new(std::ffi::OsStr::from_bytes(name.to_bytes())),
                                home,
                                seen,
                                out,
                            );
                        }
                    }
                    libc::globfree(&mut glob);
                }
            }
        }
    }
}
fn system_profiles() -> Vec<Profile> {
    let home = home_dir();
    let mut aliases = Vec::new();
    aliases_in(
        &home.join(".ssh/config"),
        &home,
        &mut HashSet::new(),
        &mut aliases,
    );
    aliases
        .into_iter()
        .filter_map(|alias| {
            let output = Command::new("/usr/bin/ssh")
                .args(["-G", "-o", "CanonicalizeHostname=no", &alias])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| profile(&alias, &String::from_utf8_lossy(&output.stdout)))
                .flatten()
        })
        .collect()
}

pub(super) fn for_reconnect(server: &SftpServer, _use_system: bool) -> SftpServer {
    let mut result = server.clone();
    if server.control_path.is_some()
        || !server.ssh_alias.is_empty()
        || server.use_password
        || !server.key.is_empty()
    {
        return result;
    }
    // Compatibility for favourites created before the alias was saved. Cache
    // only nonsecret routing for 30s; never retain a terminal's live mux socket.
    static CACHE: OnceLock<Mutex<Option<(Instant, Vec<Profile>)>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if cache
        .as_ref()
        .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(30))
    {
        *cache = Some((Instant::now(), system_profiles()));
    }
    if let Some(alias) = matching_alias(server, &cache.as_ref().unwrap().1) {
        result.ssh_alias = alias.into();
    }
    result
}

pub(super) fn error_message(stderr: &str) -> String {
    let lines: Vec<_> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines
        .iter()
        .rev()
        .find(|l| !l.eq_ignore_ascii_case("connection closed") && !l.starts_with("Warning: "))
        .or_else(|| lines.last())
        .copied()
        .unwrap_or("SSH connection failed")
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_favourite_recovers_unique_alias_with_original_port_and_user() {
        let server = SftpServer {
            host: "192.0.2.1".into(),
            user: "oliw".into(),
            port: 56883,
            ..Default::default()
        };
        let a = profile(
            "lmweb",
            "hostname 192.0.2.1\nuser oliw\nport 56883\nidentityfile /fixture/key\n",
        )
        .unwrap();
        assert_eq!(matching_alias(&server, &[a.clone()]), Some("lmweb"));
        let other = Profile {
            alias: "other-auth".into(),
            ..a.clone()
        };
        assert_eq!(matching_alias(&server, &[a.clone(), other]), None);
        assert_eq!(
            matching_alias(
                &SftpServer {
                    port: 22,
                    ..server.clone()
                },
                &[a.clone()]
            ),
            None
        );
        let restored = SftpServer {
            ssh_alias: a.alias,
            ..server
        };
        let encoded = serde_json::to_string(&restored).unwrap();
        let restored: SftpServer = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.connection_target(), "oliw@lmweb");
        assert!(ssh_options(&restored, true).contains(&"BatchMode=yes".into()));
        let live = SftpServer {
            control_path: Some("/tmp/fixture-mux".into()),
            ..restored
        };
        assert_eq!(live.connection_target(), "oliw@192.0.2.1");
        assert!(ssh_options(&live, true).contains(&"ProxyCommand=false".into()));
    }
    #[test]
    fn discovers_literal_aliases_in_includes_without_running_config_commands() {
        let dir = crate::tests::scratch("ssh-profile");
        let ssh = dir.join(".ssh");
        fs::create_dir_all(ssh.join("profiles")).unwrap();
        fs::write(ssh.join("config"), "Host lmweb alias-two * !blocked\nInclude profiles/*\nMatch exec dangerous\n ProxyCommand never-run\n").unwrap();
        fs::write(
            ssh.join("profiles/one"),
            "host=\"test-server\" # ignored\nInclude config\n",
        )
        .unwrap();
        let mut aliases = Vec::new();
        aliases_in(&ssh.join("config"), &dir, &mut HashSet::new(), &mut aliases);
        assert_eq!(aliases, ["lmweb", "alias-two", "test-server"]);
        assert_eq!(valid_alias("oliver@lmweb"), Some("lmweb"));
        assert_eq!(valid_alias("-oProxyCommand=bad"), None);
        assert_eq!(
            error_message("Permission denied (publickey).\r\nConnection closed\n"),
            "Permission denied (publickey)."
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
