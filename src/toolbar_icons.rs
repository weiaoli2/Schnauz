//! Small, consistent native vector icons. No emoji, image decoding or disk IO.
use super::*;
#[derive(Clone, Copy)]
pub(super) enum Icon {
    List,
    Grid,
    Search,
    Reload,
    FolderPlus,
    FilePlus,
    Trash,
    Sort,
    Share,
    Tag,
    Left,
    Right,
    Plus,
    Close,
    Columns,
    Eye,
    EyeOff,
    Terminal,
    Split,
    Preview,
    Tasks,
    Project,
    Settings,
    Storage,
    Drive,
    Globe,
    Cloud,
    Home,
    Calendar,
    Check,
    PinDocked,
    PinFloating,
}

thread_local! { static APPLE_SYMBOLS: RefCell<HashMap<(&'static str, [u32; 4]), Arc<RenderImage>>> = RefCell::new(HashMap::new()); }

pub(super) fn apple_share(color: Rgba) -> Option<Arc<RenderImage>> {
    apple_symbol("square.and.arrow.up", color)
}
fn apple_symbol(name: &'static str, color: Rgba) -> Option<Arc<RenderImage>> {
    let key = [
        color.r.to_bits(),
        color.g.to_bits(),
        color.b.to_bits(),
        color.a.to_bits(),
    ];
    let key = (name, key);
    APPLE_SYMBOLS.with(|cache| {
        if let Some(image) = cache.borrow().get(&key) {
            return Some(image.clone());
        }
        // Keep Apple's SF Symbol silhouette, including its native proportions.
        let symbol = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(name),
            None,
        )?;
        let configuration =
            objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(48., 0.);
        let symbol = symbol.imageWithSymbolConfiguration(&configuration)?;
        let bytes = symbol.TIFFRepresentation()?;
        let mut pixels = image::load_from_memory(&bytes.to_vec()).ok()?.to_rgba8();
        for pixel in pixels.pixels_mut() {
            pixel.0 = [
                (color.b * 255.) as u8,
                (color.g * 255.) as u8,
                (color.r * 255.) as u8,
                (pixel.0[3] as f32 * color.a) as u8,
            ];
        }
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
        let mut cache = cache.borrow_mut();
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(key, image.clone());
        Some(image)
    })
}

pub(super) fn ui_icon(icon: Icon, size: f32, color: Rgba) -> AnyElement {
    let symbol = match icon {
        Icon::Share => apple_share(color),
        Icon::PinDocked => apple_symbol("pin", color),
        Icon::PinFloating => apple_symbol("pin.slash", color),
        _ => None,
    };
    if let Some(image) = symbol {
            return img(ImageSource::Render(image))
                .w(px(size))
                .h(px(size))
                .object_fit(ObjectFit::Contain)
                .flex_none()
                .into_any_element();
    }
    gpui::canvas(
        |bounds, _, _| bounds,
        move |_, bounds, window, _| {
            let scale = f32::from(bounds.size.width) / 20.;
            let p = |x: f32, y: f32| bounds.origin + point(px(x * scale), px(y * scale));
            let mut path = gpui::PathBuilder::stroke(px(1.5 * scale));
            macro_rules! line { ($($x:expr, $y:expr);+ $(;)?) => {{
            let points = [$(p($x as f32, $y as f32)),+];
            path.move_to(points[0]);
            for point in &points[1..] { path.line_to(*point); }
        }}; }
            macro_rules! rect {
                ($x:expr,$y:expr,$w:expr,$h:expr) => {{
                    let (x, y, w, h) = ($x as f32, $y as f32, $w as f32, $h as f32);
                    path.move_to(p(x + 1.5, y));
                    path.line_to(p(x + w - 1.5, y));
                    path.cubic_bezier_to(p(x + w, y + 1.5), p(x + w, y), p(x + w, y));
                    path.line_to(p(x + w, y + h - 1.5));
                    path.cubic_bezier_to(p(x + w - 1.5, y + h), p(x + w, y + h), p(x + w, y + h));
                    path.line_to(p(x + 1.5, y + h));
                    path.cubic_bezier_to(p(x, y + h - 1.5), p(x, y + h), p(x, y + h));
                    path.line_to(p(x, y + 1.5));
                    path.cubic_bezier_to(p(x + 1.5, y), p(x, y), p(x, y));
                }};
            }
            macro_rules! circle {
                ($x:expr,$y:expr,$r:expr) => {{
                    let (x, y, r) = ($x as f32, $y as f32, $r as f32);
                    path.move_to(p(x + r, y));
                    path.arc_to(
                        point(px(r * scale), px(r * scale)),
                        px(0.),
                        true,
                        true,
                        p(x - r, y),
                    );
                    path.arc_to(
                        point(px(r * scale), px(r * scale)),
                        px(0.),
                        true,
                        true,
                        p(x + r, y),
                    );
                }};
            }
            match icon {
                Icon::PinDocked | Icon::PinFloating => {
                    line!(7,3;13,3;12,8;15,11;5,11;8,8;7,3);
                    line!(10,11;10,17);
                    if matches!(icon, Icon::PinFloating) { line!(3,3;17,17); }
                },
                Icon::Check => { line!(4,10;8,14;16,5); }
                Icon::Calendar => {
                    rect!(3, 4, 14, 13);
                    line!(3,8;17,8);
                    line!(7,2;7,6);
                    line!(13,2;13,6);
                    line!(7,11;10,11);
                }
                Icon::List => {
                    for y in [5, 10, 15] {
                        rect!(3, y - 1, 2, 2);
                        line!(8,y;17,y);
                    }
                }
                Icon::Grid => {
                    for x in [3, 12] {
                        for y in [3, 12] {
                            rect!(x, y, 5, 5);
                        }
                    }
                }
                Icon::Globe => {
                    circle!(10, 10, 8);
                    line!(2,10;18,10);
                    path.move_to(p(10., 2.));
                    path.cubic_bezier_to(p(10., 18.), p(4., 5.), p(4., 15.));
                    path.move_to(p(10., 2.));
                    path.cubic_bezier_to(p(10., 18.), p(16., 5.), p(16., 15.));
                }
                Icon::Cloud => {
                    path.move_to(p(5., 16.));
                    path.cubic_bezier_to(p(5., 10.), p(0., 16.), p(0., 10.));
                    path.cubic_bezier_to(p(15., 9.), p(5., 2.), p(14., 2.));
                    path.cubic_bezier_to(p(16., 16.), p(20., 8.), p(20., 16.));
                    path.line_to(p(5., 16.));
                }
                Icon::Home => {
                    line!(2,9;10,2;18,9);
                    line!(4,8;4,18;16,18;16,8);
                    rect!(8, 12, 4, 6);
                }
                Icon::Drive => {
                    line!(2,12;5,4;15,4;18,12);
                    rect!(2, 12, 16, 5);
                    line!(5,14.5;6,14.5);
                    line!(9,14.5;10,14.5);
                }
                Icon::Search => {
                    circle!(8.5, 8.5, 5.5);
                    line!(12.5,12.5;17,17);
                }
                Icon::Reload => {
                    path.move_to(p(16., 7.));
                    path.cubic_bezier_to(p(10., 3.), p(15., 4.5), p(12.5, 3.));
                    path.cubic_bezier_to(p(3., 10.), p(6., 3.), p(3., 6.));
                    path.cubic_bezier_to(p(10., 17.), p(3., 14.), p(6., 17.));
                    path.cubic_bezier_to(p(17., 11.), p(13.5, 17.), p(16.5, 14.5));
                    line!(16,2.5;16,7;11.5,7);
                }
                Icon::FilePlus => {
                    line!(4,2;12,2;16,6;16,18;4,18;4,2);
                    line!(12,2;12,6;16,6);
                    line!(10,9;10,15);
                    line!(7,12;13,12);
                }
                Icon::FolderPlus => {
                    // Closed folder silhouette; the plus stays within the body
                    // and every stroke has clearance from the icon's bounds.
                    line!(2.5,16.5;17.5,16.5;17.5,6.5;9,6.5;7,4.5;2.5,4.5;2.5,16.5);
                    line!(10,8.75;10,14.25);
                    line!(7.25,11.5;12.75,11.5);
                }
                Icon::Trash => {
                    line!(3,5;17,5);
                    line!(7,5;7,2.5;13,2.5;13,5);
                    line!(5,5;6,17;14,17;15,5);
                    line!(8,8;8.5,14);
                    line!(12,8;11.5,14);
                }
                Icon::Sort => {
                    line!(5,3;5,17);
                    line!(2,6;5,3;8,6);
                    line!(11,5;18,5);
                    line!(11,10;16,10);
                    line!(11,15;14,15);
                }
                Icon::Share => {
                    line!(6,8;3,8;3,17;17,17;17,8;14,8);
                    line!(10,12;10,2);
                    line!(6.5,5.5;10,2;13.5,5.5);
                }
                Icon::Tag => {
                    line!(2,3;10,3;18,11;11,18;2,9;2,3);
                    circle!(6, 7, 1.2);
                }
                Icon::Left => line!(12,4;6,10;12,16),
                Icon::Right => line!(8,4;14,10;8,16),

                Icon::Plus => {
                    line!(10,4;10,16);
                    line!(4,10;16,10);
                }
                Icon::Close => {
                    line!(5,5;15,15);
                    line!(15,5;5,15);
                }
                Icon::Columns => {
                    rect!(2, 4, 16, 12);
                    line!(8,4;8,16);
                    line!(13,4;13,16);
                }
                Icon::Eye | Icon::EyeOff => {
                    path.move_to(p(1., 10.));
                    path.cubic_bezier_to(p(19., 10.), p(5., 1.), p(15., 1.));
                    path.cubic_bezier_to(p(1., 10.), p(15., 19.), p(5., 19.));
                    circle!(10, 10, 2.5);
                    if matches!(icon, Icon::EyeOff) {
                        line!(3,3;17,17);
                    }
                }
                Icon::Terminal => {
                    rect!(2, 3, 16, 14);
                    line!(5,7;8,10;5,13);
                    line!(11,13;15,13);
                }
                Icon::Split => {
                    rect!(2, 3, 16, 14);
                    line!(10,3;10,17);
                }
                Icon::Preview => {
                    rect!(2, 3, 16, 14);
                    line!(12,3;12,17);
                    line!(5,7;9,7);
                    line!(5,10;9,10);
                }
                Icon::Tasks => {
                    line!(2,5;4,7;7,3);
                    line!(10,5;18,5);
                    line!(2,11;4,13;7,9);
                    line!(10,11;18,11);
                    line!(3,17;6,17);
                    line!(10,17;18,17);
                }
                Icon::Project => {
                    line!(2,7;2,5;8,5;10,7;18,7;18,17;2,17;2,7);
                    line!(2,9;18,9);
                }
                Icon::Storage => {
                    rect!(2, 3, 16, 14);
                    line!(10,3;10,17);
                    line!(10,10;18,10);
                    line!(2,12;10,12);
                }
                Icon::Settings => {
                    for (y, x) in [(5, 7), (10, 13), (15, 8)] {
                        line!(2,y;x-2,y);
                        line!(x+2,y;18,y);
                        circle!(x, y, 2);
                    }
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size(px(size))
    .flex_none()
    .into_any_element()
}

pub(super) fn toolbar_button(
    id: &'static str,
    pane: usize,
    icon: Icon,
    selected: bool,
    enabled: bool,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    let t = theme();
    let color = if !enabled {
        Theme::alpha(t.text_muted, 0x66)
    } else {
        rgb(if selected { t.accent } else { t.text_muted })
    };
    div()
        .id((id, pane))
        .flex_none()
        .w(px(28.))
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .when(selected, |d| d.bg(rgb(t.selected)))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|s| s.bg(rgb(t.hover)))
        })
        .tooltip(tip(label))
        .child(ui_icon(icon, 17., color))
}
