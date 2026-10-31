//! AppKit menus live on the screen, rather than inside the browser viewport.
use super::*;
use objc2::{rc::Retained, DeclaredClass};
use objc2_app_kit::{NSEventModifierFlags, NSMenu, NSMenuItem, NSView};
use objc2_foundation::{NSPoint, NSSize};
use std::cell::Cell;

thread_local! {
    static APP_ICONS: RefCell<HashMap<PathBuf, Retained<NSImage>>> = RefCell::new(HashMap::new());
    static TAG_DOTS: RefCell<HashMap<u32, Retained<NSImage>>> = RefCell::new(HashMap::new());
}

/// Native, full-colour menu images, cached at Retina resolution. Non-template
/// images retain their tag colour when AppKit highlights a menu row.
pub(super) fn tag_dot(color: u32) -> Option<Retained<NSImage>> {
    TAG_DOTS.with(|cache| {
        if let Some(image) = cache.borrow().get(&color) { return Some(image.clone()); }
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(), std::ptr::null_mut(), 32, 32, 8, 4, true,
                false, NSDeviceRGBColorSpace, 128, 32,
            )
        }?;
        let ptr = rep.bitmapData();
        if ptr.is_null() { return None; }
        // The bitmap owns this 32×32, interleaved RGBA buffer. Its initializer
        // uses premultiplied alpha, including the antialiased circle edge.
        let pixels = unsafe { std::slice::from_raw_parts_mut(ptr, 32 * 128) };
        for y in 0..32 {
            for x in 0..32 {
                let distance = ((x as f32 + 0.5 - 16.).powi(2) + (y as f32 + 0.5 - 16.).powi(2)).sqrt();
                let alpha = (11.5 - distance).clamp(0., 1.);
                let at = (y * 32 + x) * 4;
                pixels[at..at + 4].copy_from_slice(&[
                    (((color >> 16) & 255) as f32 * alpha).round() as u8,
                    (((color >> 8) & 255) as f32 * alpha).round() as u8,
                    ((color & 255) as f32 * alpha).round() as u8,
                    (255. * alpha).round() as u8,
                ]);
            }
        }
        let size = NSSize::new(16., 16.);
        rep.setSize(size);
        let image = NSImage::initWithSize(NSImage::alloc(), size);
        image.addRepresentation(&rep);
        image.setTemplate(false);
        let mut cache = cache.borrow_mut();
        if cache.len() >= 32 { cache.clear(); }
        cache.insert(color, image.clone());
        Some(image)
    })
}

#[derive(Default)]
struct TargetState {
    selected: Cell<Option<usize>>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzContextMenuTarget"]
    #[ivars = TargetState]
    struct Target;
    unsafe impl NSObjectProtocol for Target {}
    impl Target {
        #[unsafe(method(choose:))]
        fn choose(&self, sender: &NSMenuItem) { self.ivars().selected.set(Some(sender.tag() as usize)); }
    }
);

fn build(
    nodes: &[MenuNode],
    target: &Target,
    actions: &mut Vec<MenuOp>,
    mtm: objc2::MainThreadMarker,
) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(""));
    menu.setAutoenablesItems(false);
    for node in nodes {
        if matches!(node, MenuNode::Sep) {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            continue;
        }
        let (label, enabled) = match node {
            MenuNode::Item { label, .. } | MenuNode::Sub { label, .. } => (label.as_str(), true),
            MenuNode::Disabled(label) => (label.as_str(), false),
            MenuNode::Sep => unreachable!(),
        };
        // Retained menu and target outlive AppKit's tracking loop. Callbacks only
        // record an index; GPUI actions run after that loop has returned.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(label),
                None,
                &NSString::from_str(""),
            )
        };
        item.setEnabled(enabled);
        match node {
            MenuNode::Item {
                on,
                checked,
                hint,
                dot,
                app_icon,
                ..
            } => {
                if let Some(image) = dot.and_then(tag_dot) {
                    item.setImage(Some(&image));
                }
                if let Some(path) = app_icon {
                    let image = APP_ICONS.with(|cache| {
                        if let Some(image) = cache.borrow().get(path) { return image.clone(); }
                        let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path.to_string_lossy()));
                        image.setSize(NSSize::new(16., 16.));
                        image.setTemplate(false);
                        let mut cache = cache.borrow_mut();
                        if cache.len() >= 128 { cache.clear(); }
                        cache.insert(path.clone(), image.clone());
                        image
                    });
                    item.setImage(Some(&image));
                }
                if let Some(hint) = hint {
                    let key = hint
                        .chars()
                        .last()
                        .map(|c| match c {
                            '↩' => "\r".into(),
                            '⌫' => "\u{8}".into(),
                            '␣' => " ".into(),
                            c => c.to_lowercase().collect::<String>(),
                        })
                        .unwrap_or_default();
                    let mut modifiers = NSEventModifierFlags::empty();
                    if hint.contains('⌘') {
                        modifiers |= NSEventModifierFlags::Command;
                    }
                    if hint.contains('⇧') {
                        modifiers |= NSEventModifierFlags::Shift;
                    }
                    if hint.contains('⌥') {
                        modifiers |= NSEventModifierFlags::Option;
                    }
                    if hint.contains('⌃') {
                        modifiers |= NSEventModifierFlags::Control;
                    }
                    item.setKeyEquivalent(&NSString::from_str(&key));
                    item.setKeyEquivalentModifierMask(modifiers);
                }
                item.setTag(actions.len() as isize);
                item.setState(if *checked { 1 } else { 0 });
                unsafe {
                    item.setTarget(Some(target));
                    item.setAction(Some(objc2::sel!(choose:)));
                }
                actions.push(on.clone());
            }
            MenuNode::Sub { items, .. } => {
                item.setSubmenu(Some(&build(items, target, actions, mtm)))
            }
            _ => {}
        }
        menu.addItem(&item);
    }
    menu
}

fn show(nodes: &[MenuNode], view: &NSView, x: f32, y: f32) -> Option<MenuOp> {
    let mtm = objc2::MainThreadMarker::new()?;
    let allocated = Target::alloc(mtm).set_ivars(TargetState::default());
    let target: Retained<Target> = unsafe { objc2::msg_send![super(allocated), init] };
    let mut actions = Vec::new();
    let menu = build(nodes, &target, &mut actions, mtm);
    let y = if view.isFlipped() {
        y as f64
    } else {
        view.bounds().size.height - y as f64
    };
    menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(x as f64, y), Some(view));
    target
        .ivars()
        .selected
        .get()
        .and_then(|i| actions.get(i).cloned())
}

impl Schnauz {
    pub(super) fn schedule_native_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.native_menu_pending {
            return;
        }
        let pending = if let Some(menu) = &self.context_menu {
            Some((menu.x, menu.y, menu.nodes.clone()))
        } else if let Some((pane, x, y)) = self.sort_menu {
            let tab = self.tab(pane);
            let nodes = [
                SortKey::None,
                SortKey::Name,
                SortKey::Kind,
                SortKey::Modified,
                SortKey::Created,
                SortKey::Size,
                SortKey::Extension,
                SortKey::Permissions,
            ]
            .into_iter()
            .map(|key| {
                mi_check(key.label(), tab.sort_key == key, move |this, _, cx| {
                    this.set_sort(pane, key, cx)
                })
            })
            .collect();
            Some((x, y, nodes))
        } else if let Some((x, y, target)) = &self.sidebar_menu {
            Some((*x, *y, self.sidebar_menu_nodes(target.clone(), cx)))
        } else {
            None
        };
        let Some((x, y, nodes)) = pending else { return };
        let Some(handle) = window.window_handle().downcast::<Schnauz>() else {
            return;
        };
        let Some(ptr) = ns_view_ptr(window) else {
            return;
        };
        let Some(view) = (unsafe { Retained::retain(ptr.cast::<NSView>()) }) else {
            return;
        };
        self.native_menu_pending = true;
        cx.spawn(async move |_, cx| {
            // Yield out of rendering/event dispatch before AppKit starts its
            // nested tracking loop. No GPUI entity/window borrow is held here.
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
            let action = show(&nodes, &view, x, y);
            let _ = handle.update(cx, |this, window, cx| {
                this.native_menu_pending = false;
                this.context_menu = None;
                this.sort_menu = None;
                this.sidebar_menu = None;
                if let Some(action) = action {
                    action(this, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(feature = "ui-checks")]
pub(super) fn check_application_icon(path: &Path) {
    let mtm = objc2::MainThreadMarker::new().unwrap();
    let allocated = Target::alloc(mtm).set_ivars(TargetState::default());
    let target: Retained<Target> = unsafe { objc2::msg_send![super(allocated), init] };
    let mut node = mi("Fixture Viewer", |_, _, _| {});
    if let MenuNode::Item { app_icon, .. } = &mut node { *app_icon = Some(path.into()); }
    let mut actions = Vec::new();
    let menu = build(&[node], &target, &mut actions, mtm);
    let item = menu.itemAtIndex(0).unwrap();
    let image = item.image().expect("Native menu retains the application image");
    assert_eq!(image.size(), NSSize::new(16., 16.));
    assert!(!image.isTemplate());
    assert_eq!(actions.len(), 1);
}
