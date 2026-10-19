//! Credits and optional support, opened explicitly from Schnauz's app menu.
use super::*;

const PAYPAL_ADDRESS: &str = "o@freedom-net.de";
const COFFEE_URL: &str = "https://www.paypal.com/cgi-bin/webscr?cmd=_donations&business=o%40freedom-net.de&item_name=Coffee%20for%20Cookie%20-%20Schnauz&amount=2.50&currency_code=USD&no_shipping=1";

struct About {
    focus: FocusHandle,
    copied: bool,
}

fn resource(name: &str, fallback: &str) {
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent()?.parent().map(|p| p.join("Resources").join(name)));
    let path = packaged
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(fallback));
    let _ = Command::new("open").arg(path).spawn();
}

impl Render for About {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        static LOGO: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();
        let logo = LOGO
            .get_or_init(|| decode_icon(include_bytes!("../assets/Schnauz-icon.png")))
            .clone();
        static COOKIE: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();
        let cookie = COOKIE
            .get_or_init(|| decode_image(include_bytes!("../assets/Cookie-portrait.png"), 384))
            .clone();
        div().id("about-schnauz").size_full().overflow_y_scroll().flex().flex_col().p_5().gap_3()
            .bg(rgb(t.bg)).text_color(rgb(t.text)).text_size(px(13.)).track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" || (event.keystroke.modifiers.platform && event.keystroke.key == "w") {
                    window.remove_window(); cx.stop_propagation();
                }
            }))
            .child(div().flex_none().flex().items_center().gap_4()
                .children(logo.map(|logo| img(ImageSource::Render(logo)).size(px(72.)).object_fit(ObjectFit::Contain)))
                .child(div().flex().flex_col().gap_1()
                    .child(div().text_size(px(28.)).font_weight(FontWeight::BOLD).child("Schnauz"))
                    .child(div().text_color(rgb(t.text_muted)).text_size(px(11.)).child(format!("Version {} · Made for Mac", env!("CARGO_PKG_VERSION"))))))
            .child(updates::controls(f32::from(window.viewport_size().width)-40.,cx))
            .child(div().flex_none().flex().flex_col().gap_1()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Developed by Oliver Weber"))
                .child(div().text_size(px(11.)).text_color(rgb(t.text_muted)).child("Design & product direction · Oliver Weber"))
                .child(div().text_size(px(11.)).text_color(rgb(t.text_muted)).child("Implementation assistance · Codex")))
            .child(div().flex_none().flex().flex_col().gap_2().p_3().rounded_lg().bg(rgb(t.sidebar)).border_1().border_color(rgb(t.border))
                .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("Coffee for Cookie"))
                .child(div().flex().items_center().gap_3()
                    .children(cookie.map(|cookie| div().flex_none().size(px(112.)).rounded_lg().overflow_hidden()
                        .child(img(ImageSource::Render(cookie)).size_full().object_fit(ObjectFit::Contain))))
                    .child(div().flex_1().min_w_0().child("Cookie, Oliver’s schnauzer, appreciates good food. Enjoying Schnauz? A small coffee keeps Oliver coding and Cookie’s bowl full.")))
                .child(div().id("cookie-coffee").h(px(36.)).flex().items_center().justify_center().rounded_md().cursor_pointer()
                    .bg(rgb(t.accent)).text_color(rgb(0xffffff)).font_weight(FontWeight::SEMIBOLD)
                    .hover(|d| d.bg(Theme::alpha(t.accent, 0xdd)))
                    .child("Coffee for Cookie · US$2.50")
                    .on_click(|_, _, cx| cx.open_url(COFFEE_URL)))
                .child(div().text_size(px(11.)).text_color(rgb(t.text_muted)).child("Optional support via PayPal"))
                .child(div().flex().items_center().gap_2()
                    .child(div().flex_1().min_w_0().child(format!("PayPal: {PAYPAL_ADDRESS}")))
                    .child(div().id("copy-paypal-address").px_2().py_1().rounded_md().cursor_pointer().text_size(px(11.))
                        .text_color(rgb(t.accent)).hover(|d| d.bg(rgb(t.hover)))
                        .child(if self.copied { "Copied" } else { "Copy address" })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(PAYPAL_ADDRESS.into()));
                            this.copied = true; cx.notify();
                        })))))
            .child(div().flex_none().flex().flex_col().gap_2().text_size(px(11.)).text_color(rgb(t.text_muted))
                .child("Based on Shuffle by Jaime Guzman (WizenPainter), MIT license.")
                .child(div().flex().gap_4()
                    .child(div().id("about-credits").cursor_pointer().text_color(rgb(t.accent)).child("Credits")
                        .on_click(|_, _, _| resource("Credits.txt", "CREDITS.txt")))
                    .child(div().id("about-licenses").cursor_pointer().text_color(rgb(t.accent)).child("Licenses")
                        .on_click(|_, _, _| resource("Licenses", "LICENSE")))))
            .child(div().flex_none().flex().justify_end()
                .child(div().id("about-close").h(px(28.)).px_4().flex().items_center().rounded_md().cursor_pointer()
                    .bg(rgb(t.surface)).border_1().border_color(rgb(t.border)).hover(|d| d.bg(rgb(t.hover))).child("Close")
                    .on_click(|_, window, _| window.remove_window())))
    }
}

pub(super) fn open_about(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<About>() {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(520.), px(600.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(460.), px(500.))),
            titlebar: Some(TitlebarOptions {
                title: Some("About Schnauz".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| {
                cx.observe_global::<ThemeGlobal>(|_, cx| cx.notify())
                    .detach();
                cx.observe_global::<PrefsGlobal>(|_,cx|cx.notify()).detach();
                cx.observe_global::<updates::State>(|_,cx|cx.notify()).detach();
                About {
                    focus: cx.focus_handle(),
                    copied: false,
                }
            });
            window.focus(&view.read(cx).focus);
            view
        },
    );
    cx.activate(true);
}

#[cfg(feature = "ui-checks")]
pub(super) fn check_updates(cx: &mut App) {
    let window=cx.open_window(WindowOptions {
        show:false,focus:false,
        window_bounds:Some(WindowBounds::Windowed(Bounds::new(point(px(-4000.),px(-4000.)),size(px(520.),px(600.))))),
        ..Default::default()
    },|_,cx|cx.new(|cx|About {focus:cx.focus_handle(),copied:false})).unwrap();
    for _ in 0..3 {cx.update_window(*window,|_,w,cx|{w.refresh();w.draw(cx).clear();}).unwrap();}
    let source=ui_checks::recorded_text("update-source");
    assert_eq!(source.text(),"Updates are checked via GitHub.");
    assert!(f32::from(source.bounds().size.width)>100.);
    assert!(f32::from(ui_checks::recorded_bounds("update-controls").size.width)>400.);
    if cx.global::<updates::State>().ahead_of.is_some() {
        assert!(ui_checks::recorded_text("update-status").text().contains("newer than the latest GitHub release"));
    }
    let original=prefs();assert!(!original.check_updates_at_startup);
    let mut p=original;p.check_updates_at_startup=true;apply_prefs(p,cx);
    assert!(load_prefs().check_updates_at_startup);
    apply_prefs(original,cx);assert!(!load_prefs().check_updates_at_startup);
    cx.update_window(*window,|_,w,_|w.remove_window()).unwrap();
}
