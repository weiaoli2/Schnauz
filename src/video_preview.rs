//! Native, initially muted AVKit playback in the file preview.
use super::*;
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

#[derive(Clone, Copy, PartialEq)]
struct Snapshot {
    aspect: f32,
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
    fn new(path: PathBuf) -> Option<Rc<Self>> {
        let mtm = objc2::MainThreadMarker::new()?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        // The selectors and ABI types below come directly from the AVKit and
        // AVFoundation SDK headers. AVPlayer loads local media asynchronously.
        unsafe {
            let player: Retained<objc2::runtime::AnyObject> =
                msg_send![class!(AVPlayer), playerWithURL: &*url];
            let _: () = msg_send![&*player, setMuted: true];
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
                    muted: true,
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
            let (aspect, status) = item
                .map(|item| {
                    let presentation: NSSize = msg_send![&*item, presentationSize];
                    let status: isize = msg_send![&*item, status];
                    let aspect = if presentation.width > 0. && presentation.height > 0. {
                        (presentation.width / presentation.height) as f32
                    } else {
                        self.state.get().aspect
                    };
                    (aspect.clamp(0.1, 10.), status)
                })
                .unwrap_or((16. / 9., 0));
            let rate: f32 = msg_send![&*self.player, rate];
            let muted: bool = msg_send![&*self.player, isMuted];
            let next = Snapshot {
                aspect,
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
    fn deactivate(&self) {
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

fn frames(
    full: Bounds<gpui::Pixels>,
    visible: Bounds<gpui::Pixels>,
    flipped: bool,
    parent_height: f64,
) -> (NSRect, NSRect) {
    let x = f32::from(visible.origin.x) as f64;
    let y = f32::from(visible.origin.y) as f64;
    let w = f32::from(visible.size.width) as f64;
    let h = f32::from(visible.size.height) as f64;
    let outer = NSRect::new(
        NSPoint::new(x, if flipped { y } else { parent_height - y - h }),
        NSSize::new(w, h),
    );
    let inner = NSRect::new(
        NSPoint::new(
            f32::from(full.origin.x) as f64 - x,
            y + h - f32::from(full.bottom()) as f64,
        ),
        NSSize::new(
            f32::from(full.size.width) as f64,
            f32::from(full.size.height) as f64,
        ),
    );
    (outer, inner)
}

impl Schnauz {
    pub(super) fn sync_video_preview(&mut self, cx: &mut Context<Self>) {
        let tab = self.active_tab();
        let wanted = (prefs().preview
            && !self.todoist.open
            && self.space_map.is_none()
            && tab.remote.is_none()
            && tab.selection.len() == 1)
            .then(|| tab.selection.iter().next().cloned())
            .flatten()
            .filter(|p| is_video(p));
        if self.video_preview.as_ref().map(|s| &s.path) != wanted.as_ref() {
            if let Some(old) = self.video_preview.take() {
                old.deactivate();
            }
            self.video_preview = wanted.and_then(Session::new);
            if let Some(session) = &self.video_preview {
                let session = Rc::downgrade(session);
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
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
        let blocked = self.confirm_delete.is_some()
            || self.info_panel.is_some()
            || self.batch_rename.is_some()
            || self.ssh_ask
            || self.server_dialog.is_some()
            || self.group_dialog.is_some()
            || self.new_text_file.is_some()
            || self.project_dialog.is_some()
            || (self.palette_open && !self.file_search.inline);
        if let Some(session) = &self.video_preview {
            session.block(blocked);
        }
    }
    pub(super) fn render_video_preview(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let session = self.video_preview.clone()?;
        let state = session.state.get();
        let t = theme();
        let width = preview_panel::preview_width(self, self.panes.len() - 1).max(1.);
        let height = (width / state.aspect).max(40.);
        let play = session.clone();
        let mute = session.clone();
        let owner = cx.entity().downgrade();
        Some(div().id("video-inspector").w_full().h_full().min_h_0().flex().flex_col().bg(rgb(t.sidebar))
            .border_l_1().border_color(rgb(t.border))
            .child(div().flex_none().flex().items_center().gap_2().px_3().py_2()
                .child(div().flex_1().min_w_0().truncate().child(path_label(&session.path)))
                .child(div().id("preview-video-play").px_2().py_1().rounded_md().cursor_pointer().bg(rgb(t.surface))
                    .child(if state.playing { "Pause" } else { "Play" })
                    .on_click(move |_, _, cx| { play.toggle_play(); let _ = owner.update(cx, |_, cx| cx.notify()); }))
                .child(div().id("preview-video-mute").px_2().py_1().rounded_md().cursor_pointer()
                    .text_color(rgb(t.accent)).tooltip(tip("Videos start muted. Click to change sound."))
                    .child(if state.muted { "Unmute" } else { "Mute" })
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| { mute.toggle_mute(); cx.notify(); }))))
            .child(if state.failed {
                div().flex_1().min_h_0().flex().items_center().justify_center().p_4()
                    .child("This video format cannot be played by macOS. Double-click the file to open it in another app.").into_any_element()
            } else {
                div().id("video-preview-scroll").flex_1().min_h_0().overflow_y_scroll()
                    .child(gpui::canvas(|bounds, _, _| bounds, move |_, bounds, window, _| session.paint(bounds, window))
                        .w_full().h(px(height)))
                    .into_any_element()
            }).into_any_element())
    }
}

/// Integration check used by --video-preview-test. No windows or profile writes.
pub(super) fn smoke_test(path: PathBuf) -> Result<String, String> {
    use objc2_foundation::{NSDate, NSRunLoop};
    let mtm = objc2::MainThreadMarker::new().ok_or("Video must initialize on the main thread")?;
    let _ = objc2_app_kit::NSApplication::sharedApplication(mtm);
    if toolbar_icons::apple_share(rgb(0x334455)).is_none() {
        return Err("Apple Share symbol failed to render".into());
    }
    let session = Session::new(path).ok_or("Cannot create the native player")?;
    session.refresh();
    if !session.state.get().muted || session.state.get().playing {
        return Err("Player must start paused and muted".into());
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
            if !session.state.get().playing || !session.state.get().muted {
                return Err("Muted playback failed".into());
            }
            session.toggle_mute();
            if session.state.get().muted {
                return Err("Unmute failed".into());
            }
            session.toggle_mute();
            session.deactivate();
            let rate: f32 = unsafe { msg_send![&*session.player, rate] };
            if rate != 0. {
                return Err("Leaving the preview did not stop playback".into());
            }
            return Ok(format!("Apple Share symbol rendered; AVKit: ready, paused/muted by default, playback/unmute/cleanup passed · aspect {:.3}", session.state.get().aspect));
        }
    }
    Err("Media did not become ready".into())
}

#[cfg(test)]
mod tests {
    use super::*;
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
