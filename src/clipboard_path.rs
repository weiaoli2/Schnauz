//! Interpret a pasted local path as navigation, without running a shell.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Destination {
    pub folder: PathBuf,
    pub selected_file: Option<PathBuf>,
}

pub(crate) fn resolve(text: &str, home: &Path) -> io::Result<Option<Destination>> {
    let Some(mut path) = parse_path(text, home) else {
        return Ok(None);
    };
    // Prefer the literal spelling, so an existing backslash in a filename is
    // preserved. Also accept the escaped spaces produced by terminal drags.
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(original_error) => {
            let spelling = path.to_string_lossy();
            if !spelling.contains('\\') {
                return Err(original_error);
            }
            let mut chars = spelling.chars();
            let mut unescaped = String::new();
            while let Some(ch) = chars.next() {
                if ch == '\\' {
                    unescaped.push(chars.next().unwrap_or(ch));
                } else {
                    unescaped.push(ch);
                }
            }
            let candidate = PathBuf::from(unescaped);
            let metadata = fs::metadata(&candidate).map_err(|_| original_error)?;
            path = candidate;
            metadata
        }
    };
    if metadata.is_dir() && !super::is_app_bundle(&path, true) {
        Ok(Some(Destination {
            folder: path,
            selected_file: None,
        }))
    } else {
        let folder = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing enclosing folder"))?
            .to_path_buf();
        Ok(Some(Destination {
            folder,
            selected_file: Some(path),
        }))
    }
}

pub(crate) fn parse_path(text: &str, home: &Path) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() || text.contains(['\0', '\r', '\n']) {
        return None;
    }
    let text = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(text);
    if text == "~" {
        return Some(home.to_path_buf());
    }
    if let Some(tail) = text.strip_prefix("~/") {
        return Some(home.join(tail));
    }
    if let Some(rest) = text.strip_prefix("file://") {
        let start = rest.find('/')?;
        let host = &rest[..start];
        if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
            return None;
        }
        let encoded = &rest[start..];
        if encoded.contains(['?', '#']) {
            return None;
        }
        let mut decoded = Vec::new();
        let mut bytes = encoded.as_bytes().iter().copied();
        while let Some(byte) = bytes.next() {
            if byte == b'%' {
                let hi = char::from(bytes.next()?).to_digit(16)?;
                let lo = char::from(bytes.next()?).to_digit(16)?;
                decoded.push((hi * 16 + lo) as u8);
            } else {
                decoded.push(byte);
            }
        }
        let decoded = String::from_utf8(decoded).ok()?;
        if decoded.contains('\0') {
            return None;
        }
        return Some(PathBuf::from(decoded));
    }
    let path = PathBuf::from(text);
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_file_and_bundle_destinations_preserve_the_requested_item() {
        let dir = super::super::tests::scratch("clipboard-path");
        let folder = dir.join("My projects 素材");
        fs::create_dir(&folder).unwrap();
        let file = folder.join("report 2026.txt");
        fs::write(&file, "report").unwrap();
        let expected_file = Destination {
            folder: folder.clone(),
            selected_file: Some(file.clone()),
        };
        for spelling in [
            file.display().to_string(),
            format!("\"{}\"", file.display()),
            file.display().to_string().replace(' ', "\\ "),
        ] {
            assert_eq!(
                resolve(&spelling, &dir).unwrap(),
                Some(Destination {
                    folder: expected_file.folder.clone(),
                    selected_file: expected_file.selected_file.clone()
                })
            );
        }
        assert_eq!(
            resolve("~/My projects 素材", &dir).unwrap(),
            Some(Destination {
                folder: folder.clone(),
                selected_file: None
            })
        );
        assert_eq!(
            resolve(&format!(" '{}'\n", folder.display()), &dir).unwrap(),
            Some(Destination {
                folder: folder.clone(),
                selected_file: None
            })
        );
        let url = format!("file://localhost{}", file.display())
            .replace(' ', "%20")
            .replace('素', "%E7%B4%A0")
            .replace('材', "%E6%9D%90");
        assert_eq!(resolve(&url, &dir).unwrap(), Some(expected_file));
        let app = dir.join("Example.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        assert_eq!(
            resolve(&app.display().to_string(), &dir).unwrap(),
            Some(Destination {
                folder: dir.clone(),
                selected_file: Some(app)
            })
        );
        let literal = folder.join("$(literal).txt");
        fs::write(&literal, "literal").unwrap();
        assert_eq!(
            resolve(&literal.display().to_string(), &dir)
                .unwrap()
                .unwrap()
                .selected_file,
            Some(literal)
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_nonpaths_and_reports_missing_paths_without_altering_files() {
        let dir = super::super::tests::scratch("clipboard-invalid");
        for text in [
            "ordinary text",
            "relative/file.txt",
            "https://example.com/file",
            "file://server/etc/hosts",
            "file:///tmp/%QQ",
            "/tmp/a\n/tmp/b",
        ] {
            assert_eq!(resolve(text, &dir).unwrap(), None, "{text}");
        }
        assert_eq!(
            resolve("file:///", &dir).unwrap(),
            Some(Destination {
                folder: PathBuf::from("/"),
                selected_file: None
            })
        );
        assert!(resolve(&dir.join("missing.txt").display().to_string(), &dir).is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }
}
