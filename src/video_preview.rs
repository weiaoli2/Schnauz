//! Native AVKit media preview. Paused initially; videos also start muted.
use super::*;
use crate::native_preview::frames;
use objc2::{class, msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::Cell;

#[link(name = "AVKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

objc2::extern_class!(
    #[unsafe(super(NSView))]
    #[name = "AVPlayerView"]
    struct PlayerView;
);

pub(super) fn is_video(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_ascii_lowercase().as_str(),
            "mov"
                | "mp4"
                | "m4v"
                | "avi"
                | "mkv"
                | "webm"
                | "mpeg"
                | "mpg"
                | "3gp"
                | "mts"
                | "m2ts"
        )
    })
}
pub(super) fn is_audio(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_ascii_lowercase().as_str(),
            "mp3"
                | "wav"
                | "flac"
                | "aac"
                | "m4a"
                | "m4b"
                | "aiff"
                | "aif"
                | "aifc"
                | "caf"
                | "ogg"
                | "opus"
        )
    })
}
pub(super) fn is_media(path: &Path) -> bool {
    is_video(path) || is_audio(path)
}
pub(super) fn is_picture(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_ascii_lowercase().as_str(),
            "jpg"
                | "jpeg"
                | "png"
                | "gif"
                | "heic"
                | "heif"
                | "webp"
                | "tif"
                | "tiff"
                | "bmp"
                | "avif"
                | "jxl"
                | "dng"
                | "cr2"
                | "cr3"
                | "nef"
                | "arw"
                | "orf"
                | "raf"
                | "rw2"
        )
    })
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Snapshot {
    aspect: f32,
    dimensions: Option<(u32, u32)>,
    muted: bool,
    playing: bool,
    ready: bool,
    failed: bool,
}

/// All references stay on the main thread. A separate clipping NSView keeps
/// video and its controls below the GPUI header while the preview scrolls.
pub(super) struct Session {
    pub path: PathBuf,
    player: Retained<objc2::runtime::AnyObject>,
    view: Retained<PlayerView>,
    clip: Retained<NSView>,
    active: Cell<bool>,
    blocked: Cell<bool>,
    state: Cell<Snapshot>,
}
impl Session {
    #[cfg(feature = "ui-checks")]
    pub(super) fn check_painted(&self) {
        assert!(!self.clip.isHidden(),"Native video must be visible before metadata finishes: state {:?}, active {}, blocked {}, frame {:?}",self.state.get(),self.active.get(),self.blocked.get(),self.clip.frame());
        let frame = self.view.frame();
        assert!(
            frame.size.width > 200.
                && frame.size.height > if is_audio(&self.path) { 60. } else { 100. },
            "Native media needs a real viewport: {frame:?}"
        );
        assert!(
            unsafe { self.clip.superview() }.is_some(),
            "Player must attach to the actual GPUI view"
        );
        if is_audio(&self.path) {
            assert!(frame.size.height <= 90., "Audio controls stay compact");
            assert!(
                !self.state.get().playing && !self.state.get().muted,
                "Audio starts paused with sound enabled for an explicit Play click"
            );
        }
    }
    #[cfg(feature = "ui-checks")]
    pub(super) fn check_deactivated(&self) {
        assert!(!self.active.get());
        assert!(self.clip.isHidden());
        assert!(unsafe { self.clip.superview() }.is_none());
        let rate: f32 = unsafe { msg_send![&*self.player, rate] };
        assert_eq!(rate, 0.);
    }
    fn new(path: PathBuf) -> Option<Rc<Self>> {
        let mtm = objc2::MainThreadMarker::new()?;
        let muted = !is_audio(&path);
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        // The selectors and ABI types below come directly from the AVKit and
        // AVFoundation SDK headers. AVPlayer loads local media asynchronously.
        unsafe {
            let asset: Retained<objc2::runtime::AnyObject> = msg_send![class!(AVURLAsset), URLAssetWithURL: &*url, options: None::<&objc2::runtime::AnyObject>];
            // The default URL initializer also waits for duration. A preview
            // needs playable frames first; don't preload optional metadata keys.
            let keys = objc2_foundation::NSArray::<NSString>::new();
            let item: Retained<objc2::runtime::AnyObject> = msg_send![class!(AVPlayerItem), playerItemWithAsset: &*asset, automaticallyLoadedAssetKeys: &*keys];
            let player: Retained<objc2::runtime::AnyObject> =
                msg_send![class!(AVPlayer), playerWithPlayerItem: &*item];
            let _: () = msg_send![&*player, setMuted: muted];
            let view: Retained<PlayerView> = msg_send![PlayerView::alloc(mtm), initWithFrame: NSRect::new(NSPoint::new(0., 0.), NSSize::new(320., 180.))];
            let _: () = msg_send![&*view, setPlayer: &*player];
            let _: () = msg_send![&*view, setControlsStyle: 1_isize]; // Inline.
            let _: () = msg_send![&*view, setVideoGravity: &*NSString::from_str("AVLayerVideoGravityResizeAspect")];
            let _: () = msg_send![&*view, setShowsFullScreenToggleButton: false];
            let _: () = msg_send![&*view, setAllowsVideoFrameAnalysis: false];
            let _: () = msg_send![&*view, setUpdatesNowPlayingInfoCenter: false];
            let clip = NSView::initWithFrame(
                NSView::alloc(mtm),
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(320., 180.)),
            );
            clip.setWantsLayer(true);
            let layer: Option<Retained<objc2::runtime::AnyObject>> = msg_send![&*clip, layer];
            if let Some(layer) = layer {
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
            clip.addSubview(&view);
            clip.setHidden(true);
            Some(Rc::new(Self {
                path,
                player,
                view,
                clip,
                active: Cell::new(true),
                blocked: Cell::new(false),
                state: Cell::new(Snapshot {
                    aspect: 16. / 9.,
                    dimensions: None,
                    muted,
                    playing: false,
                    ready: false,
                    failed: false,
                }),
            }))
        }
    }
    fn refresh(&self) -> bool {
        unsafe {
            let item: Option<Retained<objc2::runtime::AnyObject>> =
                msg_send![&*self.player, currentItem];
            let (aspect, status, dimensions) = item
                .map(|item| {
                    let presentation: NSSize = msg_send![&*item, presentationSize];
                    let status: isize = msg_send![&*item, status];
                    let aspect = if presentation.width > 0. && presentation.height > 0. {
                        (presentation.width / presentation.height) as f32
                    } else {
                        self.state.get().aspect
                    };
                    let dimensions = (presentation.width > 0. && presentation.height > 0.)
                        .then_some((
                            presentation.width.round() as u32,
                            presentation.height.round() as u32,
                        ));
                    (aspect.clamp(0.1, 10.), status, dimensions)
                })
                .unwrap_or((16. / 9., 0, None));
            let rate: f32 = msg_send![&*self.player, rate];
            let muted: bool = msg_send![&*self.player, isMuted];
            let next = Snapshot {
                aspect,
                dimensions,
                muted,
                playing: rate > 0.,
                ready: status == 1,
                failed: status == 2,
            };
            self.state.replace(next) != next
        }
    }
    fn toggle_play(&self) {
        unsafe {
            let rate: f32 = msg_send![&*self.player, rate];
            if rate > 0. {
                let _: () = msg_send![&*self.player, pause];
            } else {
                let _: () = msg_send![&*self.player, play];
            }
        }
        self.refresh();
    }
    fn toggle_mute(&self) {
        unsafe {
            let muted: bool = msg_send![&*self.player, isMuted];
            let _: () = msg_send![&*self.player, setMuted: !muted];
        }
        self.refresh();
    }
    fn block(&self, blocked: bool) {
        if self.blocked.replace(blocked) != blocked && blocked {
            self.clip.setHidden(true);
            unsafe {
                let _: () = msg_send![&*self.player, pause];
            }
        }
    }
    pub(super) fn deactivate(&self) {
        self.active.set(false);
        self.clip.setHidden(true);
        unsafe {
            let _: () = msg_send![&*self.player, pause];
        }
        self.clip.removeFromSuperview();
    }
    fn paint(&self, bounds: Bounds<gpui::Pixels>, window: &mut Window) {
        if !self.active.get() || self.blocked.get() || self.state.get().failed {
            self.clip.setHidden(true);
            return;
        }
        let visible = bounds.intersect(&window.content_mask().bounds);
        if visible.size.width <= px(1.) || visible.size.height <= px(1.) {
            self.clip.setHidden(true);
            return;
        }
        let Some(ptr) = ns_view_ptr(window) else {
            return;
        };
        let parent = unsafe { &*ptr.cast::<NSView>() };
        let (outer, inner) = frames(
            bounds,
            visible,
            parent.isFlipped(),
            parent.bounds().size.height,
        );
        if unsafe { self.clip.superview() }.is_none() {
            parent.addSubview(&self.clip);
        }
        self.clip.setFrame(outer);
        self.view.setFrame(inner);
        self.clip.setHidden(false);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.deactivate();
    }
}

impl Schnauz {
    pub(super) fn sync_video_preview(&mut self, cx: &mut Context<Self>) {
        let tab = self.active_tab();
        let wanted = (prefs().preview && tab.remote.is_none() && tab.selection.len() == 1)
            .then(|| tab.selection.iter().next().cloned())
            .flatten()
            .filter(|p| is_media(p) && !volumes::is_ejecting(p,cx));
        if self.video_preview.as_ref().map(|s| &s.path) != wanted.as_ref() {
            if let Some(old) = self.video_preview.take() {
                old.deactivate();
            }
            self.video_preview = wanted.and_then(Session::new);
            if let Some(session) = &self.video_preview {
                let session = Rc::downgrade(session);
                let started = Instant::now();
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(
                            if started.elapsed() < Duration::from_secs(1) {
                                50
                            } else {
                                250
                            },
                        ))
                        .await;
                    let Some(session) = session.upgrade() else {
                        break;
                    };
                    if !session.active.get() {
                        break;
                    }
                    let changed = session.refresh();
                    drop(session);
                    if changed && this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                })
                .detach();
            }
        }
        let blocked = self.native_preview_blocked();
        if let Some(session) = &self.video_preview {
            session.block(blocked);
        }
    }
    pub(super) fn render_video_preview(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let session = self.video_preview.clone()?;
        let state = session.state.get();
        let t = theme();
        let width = preview_panel::preview_width(self, self.panes.len() - 1).max(1.);
        let content_width = (width - 1.).max(1.);
        let audio = is_audio(&session.path);
        let height = if audio {
            88.
        } else {
            (content_width / state.aspect).max(40.)
        };
        let information = prefs().info.then(|| {
            native_preview::information(
                &session.path,
                state.dimensions.map(|(w, h)| format!("{w} × {h}")),
                width - 1.,
            )
        });
        let play = session.clone();
        let mute = session.clone();
        let owner = cx.entity().downgrade();
        Some(div().id("video-inspector").flex_none().w(px(width)).h_full().min_h_0().flex().flex_col().bg(rgb(t.sidebar))
            .border_l_1().border_color(rgb(t.border))
            .child(div().flex_none().flex().items_center().gap_2().px_3().py_2()
                .child(div().flex_1().min_w_0().truncate().child(path_label(&session.path)))
                .child(div().id("preview-video-play").px_2().py_1().rounded_md().cursor_pointer().bg(rgb(t.surface))
                    .child(if state.playing { "Pause" } else { "Play" })
                    .on_click(move |_, _, cx| { play.toggle_play(); let _ = owner.update(cx, |_, cx| cx.notify()); }))
                .child(div().id("preview-video-mute").px_2().py_1().rounded_md().cursor_pointer()
                    .text_color(rgb(t.accent)).tooltip(tip(if audio { "Mute or unmute audio." } else { "Videos start muted. Click to change sound." }))
                    .child(if state.muted { "Unmute" } else { "Mute" })
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| { mute.toggle_mute(); cx.notify(); }))))
            .child(if state.failed {
                div().flex_1().min_h_0().flex().items_center().justify_center().p_4()
                    .child(if audio { "This audio file cannot be played by macOS. Double-click the file to open it in another app." } else { "This video format cannot be played by macOS. Double-click the file to open it in another app." }).into_any_element()
            } else {
                div().id("video-preview-scroll").w(px(content_width)).flex_1().min_h_0().overflow_y_scroll()
                    .child(div().w(px(content_width)).flex_none().flex().flex_col().gap_3()
                        .when(!state.ready,|d|d.child(div().w(px(content_width)).px_3().text_size(px(UI_SMALL_FONT_SIZE)).text_color(rgb(t.text_dim)).child(if audio { "Loading audio…" } else { "Loading video…" })))
                        .child(gpui::canvas(|bounds, _, _| bounds, move |_, bounds, window, _| session.paint(bounds, window))
                            .flex_none().w(px(content_width)).h(px(height)))
                        .children(information))
                    .into_any_element()
            }).into_any_element())
    }
}

/// Integration check used by --video-preview-test. No windows or profile writes.
#[cfg(feature="ui-checks")]
pub(super) fn smoke_test(path: PathBuf) -> Result<String, String> {
    use objc2_foundation::{NSDate, NSRunLoop};
    let mtm = objc2::MainThreadMarker::new().ok_or("Video must initialize on the main thread")?;
    let _ = objc2_app_kit::NSApplication::sharedApplication(mtm);
    if toolbar_icons::apple_share(rgb(0x334455)).is_none() {
        return Err("Apple Share symbol failed to render".into());
    }
    let audio = is_audio(&path);
    let session = Session::new(path).ok_or("Cannot create the native player")?;
    session.refresh();
    if session.state.get().muted == audio || session.state.get().playing {
        return Err("Player must start paused, audio audible and video muted".into());
    }
    // Integration checks are always silent, including audio's unmuted default.
    unsafe {
        let _: () = msg_send![&*session.player, setVolume: 0_f32];
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
        session.refresh();
        if session.state.get().failed {
            return Err("AVFoundation rejected the media".into());
        }
        if session.state.get().ready {
            session.toggle_play();
            NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.15));
            session.refresh();
            if !session.state.get().playing || session.state.get().muted == audio {
                return Err("Playback or initial mute state failed".into());
            }
            session.toggle_mute();
            if session.state.get().muted != audio {
                return Err("Mute toggle failed".into());
            }
            session.toggle_mute();
            session.block(true);
            session.refresh();
            if session.state.get().playing {
                return Err("Modal blocking did not pause playback".into());
            }
            session.block(false);
            session.toggle_play();
            session.deactivate();
            let rate: f32 = unsafe { msg_send![&*session.player, rate] };
            if rate != 0. {
                return Err("Leaving the preview did not stop playback".into());
            }
            return Ok(format!("Apple Share symbol rendered; AVKit {}: ready, paused by default, initial sound state, playback/mute/modal pause/cleanup passed · aspect {:.3}", if audio { "audio" } else { "video" }, session.state.get().aspect));
        }
    }
    Err("Media did not become ready".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_files_route_to_media_without_changing_video_or_text() {
        for name in [
            "song.MP3",
            "song.m4a",
            "song.wav",
            "song.flac",
            "song.aiff",
            "song.caf",
            "book.m4b",
            "song.opus",
        ] {
            assert!(is_audio(Path::new(name)));
            assert!(is_media(Path::new(name)));
            assert!(!is_video(Path::new(name)));
        }
        assert!(is_media(Path::new("clip.mp4")));
        assert!(!is_audio(Path::new("clip.mp4")));
        assert!(!is_media(Path::new("notes.txt")));
    }
    #[test]
    fn clipped_video_retains_full_width_and_never_covers_headers() {
        let full = Bounds::new(point(px(600.), px(100.)), size(px(400.), px(800.)));
        let visible = Bounds::new(point(px(600.), px(200.)), size(px(400.), px(300.)));
        let (outer, inner) = frames(full, visible, true, 1000.);
        assert_eq!(outer.origin.y, 200.);
        assert_eq!(outer.size.height, 300.);
        assert_eq!(inner.size, NSSize::new(400., 800.));
        assert_eq!(inner.origin.y, -400.);
        let (outer, _) = frames(full, visible, false, 1000.);
        assert_eq!(outer.origin.y, 500.);
    }
}
