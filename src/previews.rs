//! Preview work, cancellation and bounded caches; ordinary browsing stays cheap.
use super::*;

/// Cache-only keys contain NUL, so no real local filename can collide with a
/// remote source. The complete nonsecret server identity is encoded, not hashed.
pub(super) fn source_key(path:&Path,server:Option<&SftpServer>)->PathBuf {
    use std::os::unix::{ffi::{OsStrExt,OsStringExt}};
    let Some(server)=server else{return path.into();};
    let identity=serde_json::to_vec(&(&server.host,&server.user,server.port,&server.ssh_alias)).unwrap();
    let mut bytes=vec![0];bytes.extend(identity);bytes.push(0);bytes.extend(path.as_os_str().as_bytes());
    PathBuf::from(OsString::from_vec(bytes))
}

struct Downloaded {
    directory: PathBuf,
    file: PathBuf,
    bytes: usize,
    fetched: Instant,
}
impl Drop for Downloaded {fn drop(&mut self){let directory=self.directory.clone();let _=std::thread::Builder::new().name("schnauz-preview-cleanup".into()).spawn(move||{let _=fs::remove_dir_all(directory);});}}
impl Downloaded {
    fn reserve(remote:&Path)->Result<Self,String> {
        use std::os::unix::fs::DirBuilderExt;
        for _ in 0..16 {
            let directory=std::env::temp_dir().join(format!("schnauz-preview-{}",objc2_foundation::NSUUID::UUID().UUIDString()));
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(())=>return Ok(Self{file:directory.join(remote.file_name().ok_or("Missing remote filename")?),directory,bytes:0,fetched:Instant::now()}),
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>continue,
                Err(e)=>return Err(e.to_string()),
            }
        }
        Err("Couldn't reserve a private preview folder.".into())
    }
}
thread_local! {
    static REMOTE_SOURCES:RefCell<Cache<PathBuf,Arc<Downloaded>>>=RefCell::new(Cache::new(12,160*1024*1024,|d|d.bytes,|_|false));
}

impl Schnauz {
    /// Watch only selected local previews; never scan a folder for preview changes.
    pub(super) fn watched_preview_paths(&self) -> HashSet<PathBuf> {
        if !prefs().preview && !prefs().info { return HashSet::new(); }
        self.panes.iter().map(Pane::active_tab)
            .filter(|t| t.remote.is_none() && (t.network.is_none() || t.network_connected) && t.selection.len() == 1)
            .flat_map(|t| t.selection.iter().cloned()).collect()
    }

    pub(super) fn invalidate_preview(&self, path: &Path) {
        if let Some(cancel) = self.preview_pending.borrow_mut().remove(path) { cancel.store(true, Ordering::Release); }
        if let Some(cancel) = self.info_pending.borrow_mut().remove(path) { cancel.store(true, Ordering::Release); }
        TEXT_PREVIEW_CACHE.with(|c| { c.borrow_mut().remove(path); });
        PREVIEW_CACHE.with(|c| { c.borrow_mut().remove(path); });
        INFO_CACHE.with(|c| { c.borrow_mut().remove(path); });
        self.pdf_pending.borrow_mut().retain(|(p,_),cancel|{if p==path{cancel.store(true,Ordering::Release);false}else{true}});
        PDF_PAGE_CACHE.with(|c| c.borrow_mut().retain(|(p, _), _| p != path));
        PDF_COUNT_CACHE.with(|c| { c.borrow_mut().remove(path); });
    }

    pub(super) fn refresh_changed_previews(&self, changed: &HashSet<PathBuf>, cx: &mut Context<Self>) {
        for path in self.watched_preview_paths() {
            if !changed.contains(&path) { continue; }
            self.invalidate_preview(&path);
            self.ensure_preview(path.clone(), false, cx);
            if prefs().preview && prefs().preview_pages && is_pdf(&path) {
                self.ensure_pdf_page(path.clone(), self.preview_page, cx);
            }
            self.ensure_info(path, cx);
            cx.notify();
        }
    }

    pub(super) fn cancel_unused_previews(&self) {
        let selected: HashSet<PathBuf> = self
            .panes
            .iter()
            .map(Pane::active_tab)
            .filter(|t| t.selection.len() == 1)
            .filter_map(|t| {
                t.anchor
                    .as_ref()
                    .map(|path| source_key(path, t.remote.as_ref()))
            })
            .collect();
        self.preview_pending.borrow_mut().retain(|path, cancel| {
            if prefs().preview && selected.contains(path) {
                true
            } else {
                cancel.store(true, Ordering::Release);
                false
            }
        });
        self.info_pending.borrow_mut().retain(|path, cancel| {
            if prefs().info && selected.contains(path) {
                true
            } else {
                cancel.store(true, Ordering::Release);
                false
            }
        });
        self.pdf_pending
            .borrow_mut()
            .retain(|(path, page), cancel| {
                if prefs().preview
                    && selected.contains(path)
                    && page.abs_diff(self.preview_page) <= 1
                {
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
            || self.info_pending.borrow().contains_key(&path)
        {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.info_pending.borrow_mut().insert(path.clone(), cancel.clone());
        cx.spawn(async move |this, cx| {
            let p = path.clone();
            let info = workers::run("schnauz-file-info",move||gather_info(&p)).await;
            let Ok(info)=info else{return;};
            let _ = this.update(cx, |this, cx| {
                if cancel.load(Ordering::Acquire) || !this.info_pending.borrow().get(&path)
                    .is_some_and(|current| Arc::ptr_eq(current, &cancel)) { return; }
                this.info_pending.borrow_mut().remove(&path);
                INFO_CACHE.with(|c| c.borrow_mut().insert(path, info));
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn ensure_preview(&self, path: PathBuf, force: bool, cx: &mut Context<Self>) {
        if video_preview::is_video(&path) {
            return;
        }
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
            let result = workers::run("schnauz-file-preview",move || {
                    let text = text_preview::read(&p).map(Arc::new);
                    let image = if text.is_none() && !worker_cancel.load(Ordering::Acquire) {
                        build_preview_cancellable(&p, &worker_cancel)
                    } else {
                        None
                    };
                    (text, image)
                }).await;
            let (text,image)=result.unwrap_or((None,None));
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
        let server = self.active_tab().remote.clone();
        let key = source_key(&path, server.as_ref());
        if !is_pdf(&path) || lookup_pdf_page(&key, page).is_some() {
            return;
        }
        let document = if server.is_some() {
            REMOTE_SOURCES.with(|c| c.borrow().get(&key).cloned())
        } else {
            None
        };
        if server.is_some() && document.is_none() {
            self.ensure_remote_preview(self.active_pane, path, 0, cx);
            return;
        }
        if self.pdf_pending.borrow().contains_key(&(key.clone(), page)) {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.pdf_pending
            .borrow_mut()
            .insert((key.clone(), page), cancel.clone());
        let source = document
            .as_ref()
            .map(|d| d.file.clone())
            .unwrap_or_else(|| path.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            if cancel.load(Ordering::Acquire) {
                return;
            }
            let wanted = this
                .update(cx, |this, _| {
                    this.active_tab()
                        .anchor
                        .as_ref()
                        .is_some_and(|p| source_key(p, this.active_tab().remote.as_ref()) == key)
                })
                .unwrap_or(false);
            if !wanted {
                let _ = this.update(cx, |this, _| {
                    if this
                        .pdf_pending
                        .borrow()
                        .get(&(key.clone(), page))
                        .is_some_and(|current| Arc::ptr_eq(current, &cancel))
                    {
                        this.pdf_pending.borrow_mut().remove(&(key, page));
                    }
                });
                return;
            }
            let stop = cancel.clone();
            let out = workers::run("schnauz-pdf-preview", move || {
                let _keep = document;
                render_pdf_page_cancellable(&source, page, &stop)
            })
            .await
            .ok()
            .flatten();
            let _ = this.update(cx, |this, cx| {
                if cancel.load(Ordering::Acquire)
                    || !this
                        .pdf_pending
                        .borrow()
                        .get(&(key.clone(), page))
                        .is_some_and(|current| Arc::ptr_eq(current, &cancel))
                {
                    return;
                }
                this.pdf_pending.borrow_mut().remove(&(key.clone(), page));
                if !this
                    .active_tab()
                    .anchor
                    .as_ref()
                    .is_some_and(|p| source_key(p, this.active_tab().remote.as_ref()) == key)
                {
                    return;
                }
                match out {
                    Some((image, count)) => {
                        insert_pdf_page(key.clone(), page, Some(image));
                        PDF_COUNT_CACHE.with(|c| c.borrow_mut().insert(key, count));
                    }
                    None => insert_pdf_page(key, page, None),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn ensure_remote_preview(
        &self,
        pane: usize,
        remote_path: PathBuf,
        size: u64,
        cx: &mut Context<Self>,
    ) {
        const LIMIT: u64 = 40 * 1024 * 1024;
        let Some(server) = self.tab(pane).remote.clone() else {
            return;
        };
        let key = source_key(&remote_path, Some(&server));
        if !prefs().preview
            || (!is_image(&remote_path) && !is_pdf(&remote_path))
            || self.preview_pending.borrow().contains_key(&key)
        {
            return;
        }
        let fresh = REMOTE_SOURCES.with(|c| {
            c.borrow().get(&key).is_some_and(|d| {
                d.fetched.elapsed() < Duration::from_secs(15)
                    && (size == 0 || d.bytes as u64 == size)
            })
        });
        if fresh && lookup_preview(&key).is_some() {
            return;
        }
        if size > LIMIT {
            PREVIEW_CACHE.with(|c| c.borrow_mut().insert(key, None));
            return;
        }
        self.invalidate_preview(&key);
        let cancel = Arc::new(AtomicBool::new(false));
        self.preview_pending
            .borrow_mut()
            .insert(key.clone(), cancel.clone());
        let use_system = prefs().ssh_use_system;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            if cancel.load(Ordering::Acquire) {
                return;
            }
            let (rp, stop) = (remote_path.clone(), cancel.clone());
            let built = workers::run("schnauz-remote-preview", move || -> Result<_, String> {
                let mut local = Downloaded::reserve(&rp)?;
                let script = format!(
                    "get {} {}",
                    sftp_path_arg(&rp.to_string_lossy())?,
                    sftp_path_arg(&local.file.to_string_lossy())?
                );
                sftp_batch_cancellable(
                    &server,
                    &script,
                    use_system,
                    None,
                    stop.clone(),
                    Some((local.file.clone(), LIMIT)),
                )?;
                if stop.load(Ordering::Acquire) {
                    return Err("Preview cancelled".into());
                }
                local.bytes = fs::metadata(&local.file).map_err(|e| e.to_string())?.len() as usize;
                if local.bytes as u64 > LIMIT {
                    return Err("Preview exceeds 40 MB.".into());
                }
                let image = build_preview_cancellable(&local.file, &stop);
                let pdf = if is_pdf(&rp) && !stop.load(Ordering::Acquire) {
                    render_pdf_page_cancellable(&local.file, 0, &stop)
                } else {
                    None
                };
                Ok((Arc::new(local), image, pdf))
            })
            .await
            .and_then(|r| r);
            let _ = this.update(cx, |this, cx| {
                if cancel.load(Ordering::Acquire)
                    || !this
                        .preview_pending
                        .borrow()
                        .get(&key)
                        .is_some_and(|pending| Arc::ptr_eq(pending, &cancel))
                {
                    return;
                }
                this.preview_pending.borrow_mut().remove(&key);
                match built {
                    Ok((document, image, pdf)) => {
                        REMOTE_SOURCES.with(|c| c.borrow_mut().insert(key.clone(), document));
                        PREVIEW_CACHE.with(|c| c.borrow_mut().insert(key.clone(), image));
                        if let Some((image, count)) = pdf {
                            insert_pdf_page(key.clone(), 0, Some(image));
                            PDF_COUNT_CACHE.with(|c| c.borrow_mut().insert(key.clone(), count));
                        }
                        if this.preview_page != 0
                            && this.active_tab().anchor.as_ref().is_some_and(|p| {
                                source_key(p, this.active_tab().remote.as_ref()) == key
                            })
                        {
                            this.ensure_pdf_page(remote_path, this.preview_page, cx);
                        }
                    }
                    Err(error) => {
                        PREVIEW_CACHE.with(|c| c.borrow_mut().insert(key, None));
                        this.set_notice(format!("Couldn't preview the remote file: {error}"), cx);
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
#[cfg(any(test,feature="ui-checks"))]
pub(super) fn render_pdf_page(path:&Path,page:usize)->Option<(Arc<RenderImage>,usize)> {
    render_pdf_page_cancellable(path,page,&AtomicBool::new(false))
}
fn render_pdf_page_cancellable(path:&Path,page:usize,cancel:&AtomicBool)->Option<(Arc<RenderImage>,usize)> {
    use objc2_foundation::{NSPoint,NSRect,NSSize};
    use std::io::Read;
    // A page flip must not multiply full-document memory across worker threads.
    static RENDER:Mutex<()>=Mutex::new(());
    let _render=RENDER.lock().ok()?;
    const PDF_LIMIT:u64=64*1024*1024;
    if cancel.load(Ordering::Acquire)||fs::metadata(path).ok()?.len()>PDF_LIMIT{return None;}
    let mut file=fs::File::open(path).ok()?;
    let mut bytes=Vec::new();let mut buffer=[0u8;8192];
    loop {
        if cancel.load(Ordering::Acquire){return None;}
        let n=file.read(&mut buffer).ok()?;if n==0{break;}
        if bytes.len().saturating_add(n)>PDF_LIMIT as usize{return None;}
        bytes.extend_from_slice(&buffer[..n]);
    }
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

/// Generate a preview image for any file via macOS QuickLook (`qlmanage -t`),
/// then decode it into a GPUI `RenderImage`. Runs off the render thread.
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
    let edge = if video_preview::is_picture(path) {
        1600
    } else {
        600
    };
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
#[derive(Clone,Default)]
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
    let times = [
        md.as_ref().and_then(|m| m.created().ok()),
        md.as_ref().and_then(|m| m.modified().ok()),
        md.as_ref().and_then(|m| m.accessed().ok()),
    ];
    let [created, modified, accessed] = times.map(format_date);
    let date_times = times.map(system_dates::parts);

    // Image dimensions + color, both from the header only (no full decode).
    let (mut dimensions, mut color) = (None, None);
    if !video_preview::is_video(path) {
        if let Ok((w, h)) = image::image_dimensions(path) {
            dimensions = Some(format!("{w} × {h}"));
            use image::ImageDecoder;
            if let Ok(reader) = image::ImageReader::open(path).and_then(|r| r.with_guessed_format())
            {
                if let Ok(decoder) = reader.into_decoder() {
                    color = Some(color_label(decoder.color_type()));
                }
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
        for cancel in self.preview_pending.get_mut().values().chain(self.pdf_pending.get_mut().values()).chain(self.info_pending.get_mut().values()) {
            cancel.store(true, Ordering::Release);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_preview_keys_separate_servers_accounts_ports_and_local_paths() {
        let path=Path::new("/tmp/report.pdf");
        let base=SftpServer{host:"first.example".into(),user:"alice".into(),port:22,..Default::default()};
        let key=source_key(path,Some(&base));
        assert_eq!(source_key(path,None),path);
        assert_ne!(key,source_key(path,None));
        for server in [SftpServer{host:"second.example".into(),..base.clone()},SftpServer{user:"bob".into(),..base.clone()},SftpServer{port:2222,..base.clone()}] {
            assert_ne!(key,source_key(path,Some(&server)));
        }
        assert_eq!(key,source_key(path,Some(&base)));
    }

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
