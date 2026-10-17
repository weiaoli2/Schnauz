//! Bounded, native text previews. Never render binary data as document text.
use std::{fs::File, io::Read, path::Path, os::unix::fs::OpenOptionsExt};

const MAX_BYTES: usize = 128 * 1024;
const MAX_LINES: usize = 2000;

#[derive(Clone)]
pub(crate) struct TextPreview {
    pub text: String,
    pub truncated: bool,
}

pub(crate) fn read(path: &Path) -> Option<TextPreview> {
    // These containers have useful visual previews despite their text syntax.
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "pdf" | "rtf" | "svg") {
        return None;
    }
    read_text(path)
}

fn read_text(path: &Path) -> Option<TextPreview> {
    let file = File::options().read(true).custom_flags(libc::O_NONBLOCK).open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let mut truncated = bytes.len() > MAX_BYTES;
    bytes.truncate(MAX_BYTES);
    let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|p| {
                if little {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        match std::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(error) if truncated && error.error_len().is_none() => {
                std::str::from_utf8(&bytes[..error.valid_up_to()])
                    .ok()?
                    .to_owned()
            }
            Err(_) => return None,
        }
    };
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{c}'))
    {
        return None;
    }
    let mut text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ");
    if let Some((end, _)) = text.match_indices('\n').nth(MAX_LINES - 1) {
        text.truncate(end);
        truncated = true;
    }
    Some(TextPreview { text, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_unicode_and_utf16_but_rejects_binary_and_folders() {
        let dir = std::env::temp_dir().join(format!("studio-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.txt");
        std::fs::write(&path, "Readable 素材\r\n\tIndented").unwrap();
        assert_eq!(read(&path).unwrap().text, "Readable 素材\n    Indented");
        let utf16: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain("你好".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        std::fs::write(&path, utf16).unwrap();
        assert_eq!(read(&path).unwrap().text, "你好");
        std::fs::write(&path, [0, 1, 255, 4]).unwrap();
        assert!(read(&path).is_none());
        assert!(read(&dir).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bounds_large_previews_without_breaking_unicode() {
        let path =
            std::env::temp_dir().join(format!("studio-text-limit-{}.txt", std::process::id()));
        std::fs::write(&path, "素".repeat(MAX_BYTES)).unwrap();
        let preview = read(&path).unwrap();
        assert!(preview.truncated);
        assert!(preview.text.len() <= MAX_BYTES);
        std::fs::remove_file(path).unwrap();
    }
}
