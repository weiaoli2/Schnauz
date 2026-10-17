//! On-demand recursive search. No startup index, shell commands or binary content.
use std::{
    fs::{self, File},
    io::{self, Read},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::{Duration, Instant},
};

use std::os::macos::fs::MetadataExt as MacMetadataExt;

pub const RESULT_LIMIT: usize = 500;
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Scope {
    Drive,
    #[default]
    CurrentFolder,
}
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Mode {
    Content,
    #[default]
    File,
}
#[derive(Clone)]
pub struct Hit {
    pub path: PathBuf,
    pub is_dir: bool,
}
#[derive(Default, Clone, Copy)]
pub struct Progress {
    pub scanned: usize,
    pub unreadable: usize,
    pub limited: bool,
}
pub struct Update {
    pub hits: Vec<Hit>,
    pub progress: Progress,
    pub done: bool,
}
pub struct Request {
    pub root: PathBuf,
    pub mode: Mode,
    pub query: String,
    pub show_hidden: bool,
    pub cancel: Arc<AtomicBool>,
}

/// Resolve the containing mounted volume, including external disks and APFS Data.
pub fn drive_root(folder: &Path) -> io::Result<PathBuf> {
    let path = std::ffi::CString::new(folder.as_os_str().as_bytes())?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    let mount = unsafe { std::ffi::CStr::from_ptr(info.f_mntonname.as_ptr()) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(mount.to_bytes())))
}

pub fn terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}
pub fn matches_name(name: &str, words: &[String]) -> bool {
    let name = name.to_lowercase();
    words.iter().all(|word| {
        if word.starts_with('.') {
            name.ends_with(word)
        } else {
            name.contains(word)
        }
    })
}
fn visible_path(path: PathBuf) -> PathBuf {
    // APFS exposes this mount through /Users, /Applications, etc. in Finder.
    path.strip_prefix("/System/Volumes/Data")
        .map(|p| Path::new("/").join(p))
        .unwrap_or(path)
}

pub fn run(request: Request, tx: Sender<Update>) {
    let words = terms(&request.query);
    if words.is_empty() || request.cancel.load(Ordering::Acquire) {
        return;
    }
    let mut progress = Progress::default();
    let mut batch = Vec::new();
    let mut matches = 0;
    let mut last = Instant::now();
    let root_device = fs::metadata(&request.root).ok().map(|m| m.dev());
    let mut directories = vec![request.root.clone()];
    'walk: while let Some(dir) = directories.pop() {
        if request.cancel.load(Ordering::Acquire) {
            return;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                progress.unreadable += 1;
                continue;
            }
        };
        for entry in entries {
            if request.cancel.load(Ordering::Acquire) {
                return;
            }
            let Ok(entry) = entry else {
                progress.unreadable += 1;
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !request.show_hidden && name.starts_with('.') {
                continue;
            }
            let Ok(md) = entry.metadata() else {
                progress.unreadable += 1;
                continue;
            };
            if !request.show_hidden && md.st_flags() & libc::UF_HIDDEN != 0 {
                continue;
            }
            let path = entry.path();
            let is_dir = md.is_dir();
            // Do not follow directory symlinks, cross volumes or enter app packages.
            if is_dir
                && !md.file_type().is_symlink()
                && Some(md.dev()) == root_device
                && !name.to_ascii_lowercase().ends_with(".app")
            {
                directories.push(path.clone());
            }
            progress.scanned += 1;
            let matched = match request.mode {
                Mode::File => matches_name(&name, &words),
                Mode::Content if md.is_file() => {
                    match content_matches(&path, &words, &request.cancel) {
                        Ok(matched) => matched,
                        Err(_) => {
                            progress.unreadable += 1;
                            false
                        }
                    }
                }
                Mode::Content => false,
            };
            if matched {
                batch.push(Hit {
                    path: visible_path(path),
                    is_dir,
                });
                matches += 1;
                if matches >= RESULT_LIMIT {
                    progress.limited = true;
                    break 'walk;
                }
            }
            if batch.len() >= 24 || last.elapsed() >= Duration::from_millis(100) {
                if tx
                    .send(Update {
                        hits: std::mem::take(&mut batch),
                        progress,
                        done: false,
                    })
                    .is_err()
                {
                    return;
                }
                last = Instant::now();
            }
        }
    }
    if !request.cancel.load(Ordering::Acquire) {
        let _ = tx.send(Update {
            hits: batch,
            progress,
            done: true,
        });
    }
}

struct TextMatcher<'a> {
    words: &'a [String],
    found: Vec<bool>,
    tail: String,
    overlap: usize,
}
impl<'a> TextMatcher<'a> {
    fn new(words: &'a [String]) -> Self {
        Self {
            words,
            found: vec![false; words.len()],
            tail: String::new(),
            overlap: words.iter().map(|w| w.chars().count()).max().unwrap_or(1),
        }
    }
    fn feed(&mut self, text: &str) -> bool {
        if text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{c}'))
        {
            return false;
        }
        let lower = self.tail.clone() + &text.to_lowercase();
        for (word, found) in self.words.iter().zip(&mut self.found) {
            *found |= lower.contains(word);
        }
        let start = lower
            .char_indices()
            .rev()
            .nth(self.overlap)
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.tail = lower[start..].to_owned();
        true
    }
    fn matched(&self) -> bool {
        self.found.iter().all(|found| *found)
    }
}

/// Stream the whole file so a binary with an ASCII header never becomes a text hit.
/// Chunk overlap finds Unicode terms across read boundaries; cancellation is checked
/// on every read. O_NONBLOCK prevents named pipes and special files from hanging.
fn content_matches(path: &Path, words: &[String], cancel: &AtomicBool) -> io::Result<bool> {
    let mut file = File::options()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Ok(false);
    }
    let mut matcher = TextMatcher::new(words);
    let mut buffer = [0u8; 64 * 1024];
    let mut pending = Vec::new();
    let mut encoding = None;
    loop {
        if cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        let n = file.read(&mut buffer)?;
        let eof = n == 0;
        pending.extend_from_slice(&buffer[..n]);
        if encoding.is_none() {
            encoding = Some(if pending.starts_with(&[0xff, 0xfe]) {
                1
            } else if pending.starts_with(&[0xfe, 0xff]) {
                2
            } else {
                0
            });
            if encoding != Some(0) {
                pending.drain(..2);
            }
        }
        match encoding.unwrap() {
            0 => {
                let valid = match std::str::from_utf8(&pending) {
                    Ok(_) => pending.len(),
                    Err(e) if !eof && e.error_len().is_none() => e.valid_up_to(),
                    Err(_) => return Ok(false),
                };
                if !matcher.feed(std::str::from_utf8(&pending[..valid]).unwrap()) {
                    return Ok(false);
                }
                pending.drain(..valid);
            }
            encoding => {
                let little = encoding == 1;
                let mut units: Vec<u16> = pending
                    .chunks_exact(2)
                    .map(|b| {
                        if little {
                            u16::from_le_bytes([b[0], b[1]])
                        } else {
                            u16::from_be_bytes([b[0], b[1]])
                        }
                    })
                    .collect();
                if !eof && units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
                    units.pop();
                }
                let Ok(text) = String::from_utf16(&units) else {
                    return Ok(false);
                };
                if !matcher.feed(&text) {
                    return Ok(false);
                }
                pending.drain(..units.len() * 2);
                if eof && !pending.is_empty() {
                    return Ok(false);
                }
            }
        }
        if eof {
            return Ok(matcher.matched());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn search(root: &Path, query: &str, mode: Mode, hidden: bool) -> Vec<PathBuf> {
        let (tx, rx) = std::sync::mpsc::channel();
        run(
            Request {
                root: root.into(),
                query: query.into(),
                mode,
                show_hidden: hidden,
                cancel: Arc::new(AtomicBool::new(false)),
            },
            tx,
        );
        rx.into_iter()
            .flat_map(|u| u.hits.into_iter().map(|h| h.path))
            .collect()
    }
    #[test]
    fn scoped_recursive_names_and_text_are_distinct_and_literal() {
        let root = crate::tests::scratch("file-search");
        let folder = root.join("folder");
        let other = root.join("other");
        fs::create_dir_all(folder.join("child")).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::write(folder.join("Test report.TXT"), "needle café").unwrap();
        fs::write(folder.join("test.txt.bak"), "unrelated").unwrap();
        fs::write(folder.join("child/test two.txt"), "unrelated").unwrap();
        fs::write(other.join("test outside.txt"), "needle café").unwrap();
        fs::write(folder.join(".test hidden.txt"), "needle café").unwrap();
        fs::write(folder.join("binary.bin"), b"needle caf\xc3\xa9\0binary").unwrap();
        fs::write(folder.join("needle café name.txt"), "unrelated").unwrap();
        let names = search(&folder, "test .txt", Mode::File, false);
        assert_eq!(names.len(), 2);
        assert!(names.iter().all(|p| p.starts_with(&folder)));
        assert_eq!(search(&folder, "tset .txt", Mode::File, false).len(), 0);
        assert_eq!(search(&folder, "test .txt", Mode::File, true).len(), 3);
        assert_eq!(
            search(&folder, "needle café", Mode::Content, false),
            vec![folder.join("Test report.TXT")]
        );
        assert_eq!(search(&root, "test .txt", Mode::File, false).len(), 3);
        assert!(drive_root(&folder).unwrap().is_absolute());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn text_stream_handles_boundaries_encodings_binary_and_cancellation() {
        let root = crate::tests::scratch("content-stream");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("large.txt");
        let cancel = AtomicBool::new(false);
        let text = "x".repeat(65534) + "素材 café needle";
        fs::write(&path, &text).unwrap();
        assert!(content_matches(&path, &terms("素材 café needle"), &cancel).unwrap());
        for little in [true, false] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in ("x".repeat(32766) + "🦮素材 needle").encode_utf16() {
                bytes.extend(if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            fs::write(&path, bytes).unwrap();
            assert!(content_matches(&path, &terms("🦮素材 needle"), &cancel).unwrap());
        }
        fs::write(
            &path,
            text.as_bytes()
                .iter()
                .copied()
                .chain([0])
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(!content_matches(&path, &terms("needle"), &cancel).unwrap());
        cancel.store(true, Ordering::Release);
        assert!(!content_matches(&path, &terms("needle"), &cancel).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
