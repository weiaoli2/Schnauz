//! Small, consistent native vector icons. No emoji, image decoding or disk IO.
use super::*;
#[derive(Clone, Copy)]
pub(super) enum Icon {
    List,
    Grid,
    Search,
    FolderPlus,
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
    Project,
    Settings,
    Storage,
}

thread_local! { static APPLE_SHARE: RefCell<HashMap<[u32; 4], Arc<RenderImage>>> = RefCell::new(HashMap::new()); }

pub(super) fn apple_share(color: Rgba) -> Option<Arc<RenderImage>> {
    let key = [
        color.r.to_bits(),
        color.g.to_bits(),
        color.b.to_bits(),
        color.a.to_bits(),
    ];
    APPLE_SHARE.with(|cache| {
        if let Some(image) = cache.borrow().get(&key) {
            return Some(image.clone());
        }
        // Keep Apple's SF Symbol silhouette, including its native proportions.
        let symbol = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str("square.and.arrow.up"),
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
    if matches!(icon, Icon::Share) {
        if let Some(image) = apple_share(color) {
            return img(ImageSource::Render(image))
                .w(px(size))
                .h(px(size))
                .object_fit(ObjectFit::Contain)
                .flex_none()
                .into_any_element();
        }
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
                Icon::Search => {
                    circle!(8.5, 8.5, 5.5);
                    line!(12.5,12.5;17,17);
                }
                Icon::FolderPlus => {
                    line!(3,16;2,16;2,5;7,5;9,7;18,7;18,10);
                    line!(8,16;18,16);
                    line!(13,11;13,19);
                    line!(9,15;17,15);
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
