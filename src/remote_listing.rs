//! OpenSSH's long directory listings. Entries contain one child name, never
//! the path printed by `ls -l /folder`; the browser owns the parent directory.
use super::*;

pub(super) fn parse(output: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let now = chrono::Local::now();
    for line in output.lines() {
        let mut rest = line.trim_start();
        let mut fields = Vec::with_capacity(8);
        for column in 0..8 {
            let Some(end) = rest.find(char::is_whitespace) else {
                break;
            };
            fields.push(&rest[..end]);
            // OpenSSH separates the final date field from the filename with
            // one space. Preserve whitespace that belongs to the filename.
            rest = if column == 7 {
                rest[end..].strip_prefix(' ').unwrap_or(&rest[end..])
            } else {
                rest[end..].trim_start()
            };
        }
        if fields.len() != 8 {
            continue;
        }
        let permissions = fields[0];
        let Some(kind) = permissions.chars().next() else {
            continue;
        };
        if permissions.len() < 10 || !matches!(kind, 'd' | '-' | 'l' | 'c' | 'b' | 'p' | 's') {
            continue;
        }
        let Some(size) = fields[4].parse().ok() else {
            continue;
        };
        let name = if kind == 'l' && fields[1] != "?" {
            rest.split_once(" -> ").map_or(rest, |(name, _)| name)
        } else {
            rest
        };
        // Explicit-path listings can contain /folder/. and /folder/.. too.
        // Examine the raw last segment before Path would normalize the dot.
        let name = name.rsplit('/').next().unwrap_or_default();
        if matches!(name, "" | "." | "..") {
            continue;
        }
        let modified = modification_date(fields[5], fields[6], fields[7], now);
        entries.push(Entry {
            name: name.into(),
            is_dir: kind == 'd',
            link: (kind == 'l').then_some(remote_links::Target::Unavailable),
            size,
            modified: modified.map(|(time, _)| time),
            modified_date_only: modified.is_some_and(|(_, date_only)| date_only),
            created: None,
            mode: None,
            loaded: true,
            hidden_flag: false,
        });
    }
    sort_default(&mut entries);
    entries
}

/// OpenSSH prints recent mtimes as month/day/hour/minute, and older or future
/// mtimes as month/day/year. Recover only that precision; never invent a birth
/// date or display midnight as though an old file's time were known.
fn modification_date(
    month: &str,
    day: &str,
    time_or_year: &str,
    now: chrono::DateTime<chrono::Local>,
) -> Option<(SystemTime, bool)> {
    use chrono::{Datelike, Local, NaiveDate, TimeZone};
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == month)? as u32
        + 1;
    let day = day.parse::<u32>().ok()?;
    if let Some((hour, minute)) = time_or_year.split_once(':') {
        let hour = hour.parse::<u32>().ok()?;
        let minute = minute.parse::<u32>().ok()?;
        // Match OpenSSH's half-year window, including dates across New Year.
        // A repeated DST minute can have two valid offsets; take the later one
        // that still lies in this window. Missing minutes remain unknown.
        let oldest = now - chrono::Duration::seconds(365 * 24 * 60 * 60 / 2 + 60);
        [now.year() - 1, now.year(), now.year() + 1]
            .into_iter()
            .filter_map(|year| {
                let naive =
                    NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, 0)?;
                let local = Local.from_local_datetime(&naive);
                [local.earliest(), local.latest()]
                    .into_iter()
                    .flatten()
                    .filter(|time| *time >= oldest && *time <= now)
                    .max()
            })
            .max()
            .map(|time| (SystemTime::from(time), false))
    } else {
        if time_or_year.len() != 4 {
            return None;
        }
        let year = time_or_year.parse::<i32>().ok()?;
        // Noon avoids midnight DST gaps. Only the calendar date is displayed.
        let naive = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(12, 0, 0)?;
        Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|time| (SystemTime::from(time), true))
    }
}

/// Keep the listing relative to its own SFTP cwd so only immediate child names
/// are returned, independently of the server's login directory.
pub(super) fn list_command(directory: &str) -> Result<String, String> {
    // -n renders the server's attributes in the local OpenSSH client instead
    // of accepting a server-supplied longname with an unknown locale/timezone.
    Ok(format!("cd {}\nls -lan", sftp_path_arg(directory)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_path_listing_contains_only_children_without_dot_or_parent_entries() {
        let output = "sftp> ls -la /home/oliw/../..\n\
            drwxr-xr-x  2 1000 1000 4096 Oct 17 14:31 /home/oliw/../../.\n\
            drwxr-xr-x  2 1000 1000 4096 Oct 17 14:31 /home/oliw/../../..\n\
            drwxr-xr-x  2 1000 1000 4096 Oct 17 14:31 /home/oliw/../../boot\n\
            drwxr-xr-x  2 1000 1000 4096 Oct 17 14:31 /home/oliw/../../etc\n\
            -rw-r--r--  1 1000 1000   42 Oct 17 14:31 /home/oliw/../../notes.txt\n";
        let entries = parse(output);
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["boot", "etc", "notes.txt"]
        );
        assert!(entries[0].is_dir && !entries[2].is_dir);
        assert_eq!(entries[2].size, 42);
    }
    #[test]
    fn relative_listing_preserves_spaces_unicode_hidden_names_and_symlink_names() {
        let output = "sftp> cd /home/oliw\nsftp> ls -la\n\
            drwxr-xr-x  2 1000 1000 4096 Oct 17 14:31 .\n\
            drwxr-xr-x  2 1000 1000 4096 Oct 17 14:31 ..\n\
            drwx------  2 1000 1000 4096 Oct 17 14:31 .ssh\n\
            -rw-r--r--  1 1000 1000   42 Oct 17 14:31   素材  notes  .txt \n\
            lrwxrwxrwx  1 1000 1000   12 Oct 17  2025 shortcut -> /elsewhere/target\n";
        let entries = parse(output);
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.name == ".ssh"));
        assert!(entries.iter().any(|e| e.name == "  素材  notes  .txt "));
        assert!(entries.iter().any(|e| e.name == "shortcut"));
        assert!(entries.iter().all(|e| !e.name.contains('/')));
    }
    #[test]
    fn directory_listing_is_scoped_and_does_not_expand_into_batch_commands() {
        assert_eq!(
            list_command("/home/oliw/folder name").unwrap(),
            "cd \"/home/oliw/folder name\"\nls -lan"
        );
        assert!(list_command("/home/oliw\nls /another").is_err());
    }
}
