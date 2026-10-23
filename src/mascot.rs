//! The Schnauz mascot opens About while idle and stops an active search.
//! Animation requests frames only during a search.
use super::*;
impl Schnauz {
    pub(super) fn render_search_mascot(&self, cx: &Context<Self>) -> AnyElement {
        let running = self.file_search.busy && self.palette_open && self.file_search.inline;
        let dog = div()
            .id("schnauz-mascot")
            .relative()
            .children(ui_checks::probe("mascot-body"))
            .w(px(28.))
            .h(px(28.))
            .flex_none()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::alpha(theme().accent, 0x30)))
            .when(running, |d| d.bg(Theme::alpha(theme().accent, 0x18)))
            .tooltip(tip(if running {
                "Stop search · keep the results found so far"
            } else {
                "About Schnauz · Coffee for Cookie"
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if this.file_search.busy && this.palette_open && this.file_search.inline {
                    this.stop_file_search(cx);
                } else {
                    about::open_about(cx);
                }
                cx.stop_propagation();
            }))
            .child(
                gpui::canvas(
                    |bounds, _, _| bounds,
                    move |_, bounds, window, _| {
                        static START: OnceLock<Instant> = OnceLock::new();
                        let phase = if running {
                            START.get_or_init(Instant::now).elapsed().as_secs_f32() * 11.
                        } else {
                            0.
                        };
                        draw_dog(bounds, running, phase, window);
                    },
                )
                .size_full(),
            )
            .when_some(updates::badge(cx),|dog,(color,message)|dog.child(
                div().id("schnauz-update-info").absolute().top(px(0.)).right(px(-2.))
                    .children(ui_checks::probe("update-info"))
                    .size(px(5.)).rounded_full().bg(rgb(color))
                    .cursor_pointer().tooltip(tip(message))
                    .on_click(|_,_,cx| {cx.stop_propagation();about::open_about(cx);})
            ));
        if running {
            dog.with_animation(
                "schnauz-running",
                Animation::new(Duration::from_millis(570)).repeat(),
                |dog, _| dog,
            )
            .into_any_element()
        } else {
            dog.into_any_element()
        }
    }
}
fn draw_dog(bounds: Bounds<gpui::Pixels>, running: bool, phase: f32, window: &mut Window) {
    let scale = f32::from(bounds.size.width) / 32.;
    let point_at = |x: f32, y: f32| bounds.origin + point(px(x * scale), px(y * scale));
    let bob = if running { phase.sin().abs() * 1.5 } else { 0. };
    let blue = 0x6286bd;
    let dark = 0x294164;
    let cream = 0xf5eedc;
    let oval = |window: &mut Window, x: f32, y: f32, w: f32, h: f32, color: u32| {
        window.paint_quad(
            gpui::fill(
                Bounds::new(point_at(x, y), size(px(w * scale), px(h * scale))),
                rgb(color),
            )
            .corner_radii(px(w.min(h) * scale / 2.)),
        );
    };
    let stroke = |window: &mut Window, points: &[(f32, f32)], width: f32, color: u32| {
        let mut path = gpui::PathBuilder::stroke(px(width * scale));
        if let Some((x, y)) = points.first() {
            path.move_to(point_at(*x, *y));
            for (x, y) in &points[1..] {
                path.line_to(point_at(*x, *y));
            }
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, rgb(color));
        }
    };
    let triangle = |window: &mut Window, points: [(f32, f32); 3], color: u32| {
        let mut path = gpui::PathBuilder::fill();
        path.move_to(point_at(points[0].0, points[0].1));
        path.line_to(point_at(points[1].0, points[1].1));
        path.line_to(point_at(points[2].0, points[2].1));
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, rgb(color));
        }
    };
    oval(window, 3., 29., 26., 1.5, 0xabb7c7);
    let (head_x, head_y) = if running {
        let step = phase.sin() * 4.;
        stroke(
            window,
            &[(8., 22. + bob), (6. + step, 27.), (3. + step, 28.)],
            2.6,
            dark,
        );
        stroke(
            window,
            &[(20., 22. + bob), (23. - step, 27.), (26. - step, 28.)],
            2.6,
            dark,
        );
        stroke(window, &[(10., 22. + bob), (10. - step, 28.)], 2.6, blue);
        stroke(window, &[(21., 22. + bob), (20. + step, 28.)], 2.6, blue);
        stroke(
            window,
            &[
                (8., 18. + bob),
                (4., 16. + phase.cos()),
                (3., 12. + phase.cos()),
            ],
            2.8,
            dark,
        );
        oval(window, 6., 15. + bob, 19., 10., blue);
        (19., 7. + bob)
    } else {
        stroke(window, &[(10., 24.), (5., 25.), (4., 22.)], 2.4, dark);
        oval(window, 9., 17., 14., 12., blue);
        oval(window, 8., 25., 7., 4., dark);
        oval(window, 19., 25., 6., 4., dark);
        stroke(window, &[(16., 19.), (15., 27.), (18., 27.)], 2.8, cream);
        (12., 5.)
    };
    triangle(
        window,
        [
            (head_x - 1., head_y + 5.),
            (head_x, head_y - 2.),
            (head_x + 5., head_y + 1.),
        ],
        dark,
    );
    triangle(
        window,
        [
            (head_x + 7., head_y + 1.),
            (head_x + 12., head_y - 1.),
            (head_x + 12., head_y + 7.),
        ],
        dark,
    );
    oval(window, head_x, head_y, 12., 13., blue);
    oval(window, head_x - 1., head_y + 8., 14., 8., cream);
    oval(window, head_x + 1., head_y + 4., 3., 2., cream);
    oval(window, head_x + 7., head_y + 4., 3., 2., cream);
    oval(window, head_x + 2., head_y + 6., 1.8, 2.2, dark);
    oval(window, head_x + 8., head_y + 6., 1.8, 2.2, dark);
    oval(window, head_x + 4., head_y + 9., 5., 3., dark);
    stroke(
        window,
        &[(head_x + 6.5, head_y + 12.), (head_x + 6.5, head_y + 14.)],
        1.2,
        dark,
    );
    if running {
        oval(window, 24., 23., 7., 7., 0xcf5d5d);
        window.paint_quad(gpui::fill(
            Bounds::new(point_at(26., 25.), size(px(3. * scale), px(3. * scale))),
            rgb(0xffffff),
        ));
    }
}
