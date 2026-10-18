//! Preview work, cancellation and bounded caches; ordinary browsing stays cheap.
use super::*;

impl Schnauz {
    pub(super) fn cancel_unused_previews(&self) {
        let wanted: HashSet<PathBuf> = self
            .panes
            .iter()
            .map(Pane::active_tab)
            .filter(|t| {
                prefs().preview && t.selection.len() == 1
            })
            .filter_map(|t| t.anchor.clone())
            .collect();
        self.preview_pending.borrow_mut().retain(|path, cancel| {
            if wanted.contains(path) {
                true
            } else {
                cancel.store(true, Ordering::Release);
                false
            }
        });
    }
    pub(super) fn ensure_info(&self, path: PathBuf, cx: &mut Context<Self>) {
        if !prefs().info
            || lookup_info(&path).is_some()
            || !self.info_pending.borrow_mut().insert(path.clone())
        {
            return;
        }
        cx.spawn(async move |this, cx| {
            let p = path.clone();
            let info = cx.background_spawn(async move { gather_info(&p) }).await;
            let _ = this.update(cx, |this, cx| {
                this.info_pending.borrow_mut().remove(&path);
                INFO_CACHE.with(|c| c.borrow_mut().insert(path, info));
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn ensure_preview(&self, path: PathBuf, force: bool, cx: &mut Context<Self>) {
        if video_preview::is_video(&path) { return; }
        if (!force && !prefs().preview)
            || lookup_preview(&path).is_some()
            || self.preview_pending.borrow().contains_key(&path)
        {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.preview_pending
            .borrow_mut()
            .insert(path.clone(), cancel.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            if cancel.load(Ordering::Acquire) {
                return;
            }
            let worker_cancel = cancel.clone();
            let p = path.clone();
            let (text, image) = cx
                .background_spawn(async move {
                    let text = text_preview::read(&p).map(Arc::new);
                    let image = if text.is_none() && !worker_cancel.load(Ordering::Acquire) {
                        build_preview_cancellable(&p, &worker_cancel)
                    } else {
                        None
                    };
                    (text, image)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let same = this
                    .preview_pending
                    .borrow()
                    .get(&path)
                    .is_some_and(|current| Arc::ptr_eq(current, &cancel));
                if !same {
                    return;
                }
                this.preview_pending.borrow_mut().remove(&path);
                TEXT_PREVIEW_CACHE.with(|c| c.borrow_mut().insert(path.clone(), text));
                PREVIEW_CACHE.with(|c| c.borrow_mut().insert(path, image));
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn ensure_pdf_page(&self, path: PathBuf, page: usize, cx: &mut Context<Self>) {
        if !is_pdf(&path)
            || lookup_pdf_page(&path, page).is_some()
            || !self.pdf_pending.borrow_mut().insert((path.clone(), page))
        {
            return;
        }
        let src = if self.active_tab().remote.is_some() {
            remote_preview_temp(&path)
        } else {
            path.clone()
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let wanted = this
                .update(cx, |this, _| {
                    this.active_tab().anchor.as_ref() == Some(&path) && this.preview_page == page
                })
                .unwrap_or(false);
            if !wanted {
                let _ = this.update(cx, |this, _| {
                    this.pdf_pending.borrow_mut().remove(&(path, page));
                });
                return;
            }
            let out = cx
                .background_spawn(async move { render_pdf_page(&src, page) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.pdf_pending.borrow_mut().remove(&(path.clone(), page));
                if this.active_tab().anchor.as_ref() != Some(&path) || this.preview_page != page {
                    return;
                }
                match out {
                    Some((image, count)) => {
                        insert_pdf_page(path.clone(), page, Some(image));
                        PDF_COUNT_CACHE.with(|c| c.borrow_mut().insert(path, count));
                    }
                    None => insert_pdf_page(path, page, None),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Preview a remote (SFTP) file: download it once to a temp cache, build the
    /// preview from that local copy, and store the result under the *remote*
    /// path so the inspector's cache lookups find it. Limited to images and
    /// PDFs under a size cap so we never pull a huge binary just to preview it.
    pub(super) fn ensure_remote_preview(
        &self,
        remote_path: PathBuf,
        size: u64,
        cx: &mut Context<Self>,
    ) {
        if !prefs().preview || lookup_preview(&remote_path).is_some() {
            return;
        }
        let Some(server) = self.active_tab().remote.clone() else {
            return;
        };
        // Only fetch types worth previewing.
        if !is_image(&remote_path) && !is_pdf(&remote_path) {
            return;
        }
        // Cap the download; mark oversized files unavailable so we don't retry.
        const MAX_PREVIEW_BYTES: u64 = 40 * 1024 * 1024;
        if size > MAX_PREVIEW_BYTES {
            PREVIEW_CACHE.with(|c| c.borrow_mut().insert(remote_path, None));
            cx.notify();
            return;
        }
        let use_system = prefs().ssh_use_system;
        let want_pdf = is_pdf(&remote_path);
        cx.spawn(async move |this, cx| {
            let s = server.clone();
            let rp = remote_path.clone();
            let built = cx
                .background_spawn(async move {
                    let local = remote_preview_temp(&rp);
                    if let Some(parent) = local.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    // Fetch a fresh copy (the remote file may have changed).
                    let r = rp.to_string_lossy().replace('"', "");
                    let l = local.to_string_lossy().replace('"', "");
                    let script = format!("get \"{r}\" \"{l}\"");
                    match sftp_batch(&s, &script, use_system) {
                        Ok(_) => {
                            let img = build_preview(&local);
                            let pdf = if want_pdf {
                                render_pdf_page(&local, 0)
                            } else {
                                None
                            };
                            Some((img, pdf))
                        }
                        Err(_) => None,
                    }
                })
                .await;
            let _ = this.update(cx, |_this, cx| {
                match built {
                    Some((img, pdf)) => {
                        PREVIEW_CACHE.with(|c| c.borrow_mut().insert(remote_path.clone(), img));
                        if let Some((pimg, count)) = pdf {
                            insert_pdf_page(remote_path.clone(), 0, Some(pimg));
                            PDF_COUNT_CACHE
                                .with(|c| c.borrow_mut().insert(remote_path.clone(), count));
                        }
                    }
                    // Download/build failed → cache "unavailable" (shows the icon).
                    None => {
                        PREVIEW_CACHE.with(|c| c.borrow_mut().insert(remote_path.clone(), None));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

thread_local! {
    /// Cache of generated file previews. `None` = generation failed/unavailable.
    pub(super) static TEXT_PREVIEW_CACHE: RefCell<Cache<PathBuf, Option<Arc<text_preview::TextPreview>>>> =
        RefCell::new(Cache::new(64, 8*1024*1024, |v|v.as_ref().map_or(1,|v|v.text.len()), |_|false));
    pub(super) static PREVIEW_CACHE: RefCell<Cache<PathBuf, Option<Arc<RenderImage>>>> =
        RefCell::new(Cache::new(48, 32*1024*1024, image_bytes, |_|false));
    /// Cache of gathered file information.
    pub(super) static INFO_CACHE: RefCell<Cache<PathBuf, FileInfo>> = RefCell::new(Cache::new(256,usize::MAX, |_|1, |_|false));
}

pub(super) fn lookup_preview(path: &Path) -> Option<Option<Arc<RenderImage>>> {
    PREVIEW_CACHE.with(|c| c.borrow().get(path).cloned())
}

thread_local! {
    /// Rendered PDF pages for the inspector pager: (path, page) → image.
    /// `None` = that page couldn't be rendered.
    pub(super) static PDF_PAGE_CACHE: RefCell<Cache<(PathBuf,usize), Option<Arc<RenderImage>>>> =
        RefCell::new(Cache::new(12,48*1024*1024,image_bytes, |_|false));
    /// Page counts of PDFs we've rendered at least one page of.
    pub(super) static PDF_COUNT_CACHE: RefCell<Cache<PathBuf,usize>> = RefCell::new(Cache::new(128,usize::MAX, |_|1, |_|false));
}

pub(super) fn lookup_pdf_page(path: &Path, page: usize) -> Option<Option<Arc<RenderImage>>> {
    PDF_PAGE_CACHE.with(|c| c.borrow().get(&(path.to_path_buf(), page)).cloned())
}

pub(super) fn lookup_pdf_count(path: &Path) -> Option<usize> {
    PDF_COUNT_CACHE.with(|c| c.borrow().get(path).copied())
}

/// Insert a rendered page, evicting far-away pages so flipping through a long
/// PDF can't accumulate unbounded image memory (each page is a few MB).
pub(super) fn insert_pdf_page(path: PathBuf, page: usize, img: Option<Arc<RenderImage>>) {
    PDF_PAGE_CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if m.len() >= 12 {
            let lo = page.saturating_sub(3);
            let hi = page + 3;
            m.retain(|(p, pg), _| p == &path && (lo..=hi).contains(pg));
        }
        m.insert((path, page), img);
    });
}

/// Rasterize one page of a PDF via AppKit's `NSPDFImageRep`, off the render
/// thread. Returns the page image and the document's page count.
pub(super) fn render_pdf_page(path: &Path, page: usize) -> Option<(Arc<RenderImage>, usize)> {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let bytes = fs::read(path).ok()?;
    let data = NSData::with_bytes(&bytes);
    let rep = NSPDFImageRep::imageRepWithData(&data)?;
    let count = rep.pageCount().max(1) as usize;
    rep.setCurrentPage(page.min(count - 1) as isize);

    let size = rep.size();
    if !size.width.is_finite() || !size.height.is_finite() || size.width < 1.0 || size.height < 1.0
    {
        return None;
    }
    // ~800px on the long edge: crisp at the inspector's 288px (incl. retina)
    // without ballooning the page cache.
    let scale = (800.0 / size.width.max(size.height)).min(4.0);
    let (w, h) = (
        (size.width * scale).round().max(1.0) as isize,
        (size.height * scale).round().max(1.0) as isize,
    );

    let bmp = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bmp)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&ctx));
    let dst = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w as f64, h as f64));
    let ok = rep.drawInRect(dst);
    NSGraphicsContext::restoreGraphicsState_class();
    if !ok {
        return None;
    }

    let tiff = bmp.TIFFRepresentation()?;
    if tiff.len() == 0 {
        return None;
    }
    let decoded = image::load_from_memory(&tiff.to_vec()).ok()?;
    let rgba = decoded.to_rgba8();
    let (dw, dh) = rgba.dimensions();
    let mut raw = rgba.into_raw();
    for px in raw.chunks_exact_mut(4) {
        px.swap(0, 2); // RGBA → BGRA
    }
    let buffer = image::RgbaImage::from_raw(dw, dh, raw)?;
    Some((
        Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])),
        count,
    ))
}

pub(super) fn lookup_info(path: &Path) -> Option<FileInfo> {
    INFO_CACHE.with(|c| c.borrow().get(path).cloned())
}

/// Deterministic local temp path for previewing a remote file. Keeps the
/// original file name (so the extension drives QuickLook/PDF rendering) and
/// hashes the full remote path so distinct files never collide.
pub(super) fn remote_preview_temp(remote: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    remote.to_string_lossy().hash(&mut h);
    let name = remote
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    std::env::temp_dir()
        .join("schnauz-remote-preview")
        .join(format!("{:016x}-{name}", h.finish()))
}

/// Generate a preview image for any file via macOS QuickLook (`qlmanage -t`),
/// then decode it into a GPUI `RenderImage`. Runs off the render thread.
pub(super) fn build_preview(path: &Path) -> Option<Arc<RenderImage>> {
    build_preview_cancellable(path, &AtomicBool::new(false))
}
struct PreviewDirectory(PathBuf);
impl Drop for PreviewDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn build_preview_cancellable(path: &Path, cancel: &AtomicBool) -> Option<Arc<RenderImage>> {
    if cancel.load(Ordering::Acquire) || !path.is_file() {
        return None;
    }
    static NEXT_PREVIEW: AtomicU64 = AtomicU64::new(1);
    let directory = PreviewDirectory(std::env::temp_dir().join(format!(
        "schnauz-preview-{}-{}",
        std::process::id(),
        NEXT_PREVIEW.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir_all(&directory.0).ok()?;
    let edge = if video_preview::is_picture(path) { 1600 } else { 600 };
    let mut child = Command::new("qlmanage")
        .args(["-t", "-s", &edge.to_string(), "-o"])
        .arg(&directory.0)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if !wait_for_child(&mut child, cancel, Duration::from_secs(10)) {
        return None;
    }
    let name = path.file_name()?.to_string_lossy();
    let png = directory.0.join(format!("{name}.png"));
    let decoded = image::open(png).ok()?.thumbnail(edge, edge);
    let rgba = decoded.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut raw = rgba.into_raw();
    for pixel in raw.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(w, h, raw)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}
fn wait_for_child(child: &mut std::process::Child, cancel: &AtomicBool, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Acquire) || start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Everything we display in the Information inspector for one file.
#[derive(Clone)]
pub(super) struct FileInfo {
    pub(super) kind: String,
    pub(super) size: String,
    pub(super) created: String,
    pub(super) modified: String,
    pub(super) accessed: String,
    pub(super) date_times: [(String, String); 3],
    pub(super) dimensions: Option<String>,
    pub(super) color: Option<String>,
    pub(super) signed: Option<String>,
}

/// Gather file information (cheap calls only; image header read, optional
/// codesign check). Safe to call off the render thread.
pub(super) fn gather_info(path: &Path) -> FileInfo {
    let md = fs::metadata(path).ok();
    let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let size = md
        .as_ref()
        .map(|m| format_size(is_dir, m.len()))
        .unwrap_or_else(|| "--".into());
    let times = [md.as_ref().and_then(|m| m.created().ok()),
        md.as_ref().and_then(|m| m.modified().ok()), md.as_ref().and_then(|m| m.accessed().ok())];
    let [created, modified, accessed] = times.map(format_date);
    let date_times = times.map(system_dates::parts);

    // Image dimensions + color, both from the header only (no full decode).
    let (mut dimensions, mut color) = (None, None);
    if let Ok((w, h)) = image::image_dimensions(path) {
        dimensions = Some(format!("{w} × {h}"));
        use image::ImageDecoder;
        if let Ok(reader) = image::ImageReader::open(path).and_then(|r| r.with_guessed_format()) {
            if let Ok(decoder) = reader.into_decoder() {
                color = Some(color_label(decoder.color_type()));
            }
        }
    }

    // Code signature (apps / binaries). Cheap-ish; only run for plausible items.
    let signed = code_signature(path, is_dir);

    FileInfo {
        kind: kind_label(
            path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            is_dir,
        ),
        size,
        created,
        modified,
        accessed,
        date_times,
        dimensions,
        color,
        signed,
    }
}

pub(super) fn color_label(c: image::ColorType) -> String {
    use image::ColorType::*;
    match c {
        L8 | L16 => "Grayscale",
        La8 | La16 => "Grayscale + Alpha",
        Rgb8 | Rgb16 | Rgb32F => "RGB",
        Rgba8 | Rgba16 | Rgba32F => "RGB + Alpha",
        _ => "Other",
    }
    .to_string()
}

/// Returns a short code-signature status for apps/executables, else `None`.
pub(super) fn code_signature(path: &Path, is_dir: bool) -> Option<String> {
    let is_app = is_app_bundle(path, is_dir);
    if !is_app {
        return None;
    }
    let out = Command::new("codesign")
        .args(["-dv", "--verbose=2"])
        .arg(path)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return is_app.then(|| "Not signed".to_string());
    }
    let authority = text
        .lines()
        .find_map(|l| l.strip_prefix("Authority="))
        .map(|a| a.to_string());
    Some(authority.unwrap_or_else(|| "Signed".to_string()))
}

impl Drop for Schnauz {
    fn drop(&mut self) {
        for cancel in self.preview_pending.get_mut().values() {
            cancel.store(true, Ordering::Release);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_preview_child_is_reaped_promptly() {
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            trigger.store(true, Ordering::Release);
        });
        let start = Instant::now();
        assert!(!wait_for_child(&mut child, &cancel, Duration::from_secs(5)));
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(!child.wait().unwrap().success());
        thread.join().unwrap();
        assert!(code_signature(Path::new("ordinary.txt"), false).is_none());
    }
}
