//! Native, scrollable rule rows. Draft edits stay local until explicit Save.
use super::*;
use objc2::{rc::Retained, DeclaredClass};
use objc2_app_kit::{
    NSAlert, NSAppearanceCustomization, NSApplication, NSBezelStyle, NSButton, NSButtonType,
    NSFont, NSPopUpButton, NSScrollView, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

#[derive(Default)]
struct EditorState {
    colors: RefCell<Vec<u32>>,
    swatches: RefCell<Vec<Vec<Retained<NSButton>>>>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SchnauzMarkRuleActions"]
    #[ivars = EditorState]
    struct Actions;
    unsafe impl NSObjectProtocol for Actions {}
    impl Actions {
        #[unsafe(method(editRows:))]
        fn edit_rows(&self, sender:&NSButton) {
            let app=NSApplication::sharedApplication(objc2::MainThreadMarker::new().expect("Mark settings on main thread"));
            app.stopModalWithCode(sender.tag());
        }
        #[unsafe(method(chooseColor:))]
        fn choose_color(&self, sender:&NSButton) {
            let index=sender.tag().max(0) as usize;
            let (row,color)=(index/6,index%6);
            let mut colors=self.ivars().colors.borrow_mut();
            let Some(current)=colors.get_mut(row) else{return;};
            *current=PALETTE[color].1;
            if let Some(buttons)=self.ivars().swatches.borrow().get(row) {
                for (i,button) in buttons.iter().enumerate() {button.setImage(swatch(PALETTE[i].1,i==color).as_deref());}
            }
        }
    }
);
thread_local! {
    static SWATCHES:RefCell<HashMap<(u32,bool),Retained<NSImage>>>=RefCell::new(HashMap::new());
}
/// Retina circles with a crisp blue selection ring; transparent between rings.
fn swatch(color: u32, selected: bool) -> Option<Retained<NSImage>> {
    SWATCHES.with(|cache| {
        if let Some(image)=cache.borrow().get(&(color,selected)){return Some(image.clone());}
        let rep=unsafe { NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),std::ptr::null_mut(),48,48,8,4,true,false,NSDeviceRGBColorSpace,192,32
        ) }?;
        let ptr=rep.bitmapData();if ptr.is_null(){return None;}
        let pixels=unsafe{std::slice::from_raw_parts_mut(ptr,48*192)};
        for y in 0..48 {for x in 0..48 {
            let distance=((x as f32+0.5-24.).powi(2)+(y as f32+0.5-24.).powi(2)).sqrt();
            let fill=(16.-distance).clamp(0.,1.);
            let ring=if selected {(distance-19.).clamp(0.,1.)*(22.-distance).clamp(0.,1.)}else{0.};
            let (shade,alpha)=if ring>0. {(CHROME_HIGHLIGHT,ring)}else{(color,fill)};
            let at=(y*48+x)*4;
            pixels[at..at+4].copy_from_slice(&[(((shade>>16)&255) as f32*alpha) as u8,(((shade>>8)&255) as f32*alpha) as u8,((shade&255) as f32*alpha) as u8,(255.*alpha) as u8]);
        }}
        let size=NSSize::new(24.,24.);rep.setSize(size);
        let image=NSImage::initWithSize(NSImage::alloc(),size);image.addRepresentation(&rep);image.setTemplate(false);
        cache.borrow_mut().insert((color,selected),image.clone());Some(image)
    })
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
fn label(text: &str, frame: NSRect, mtm: objc2::MainThreadMarker) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFrame(frame);
    label.setFont(Some(&NSFont::systemFontOfSize(11.)));
    label
}
fn popup(
    labels: &[&str],
    selected: usize,
    frame: NSRect,
    mtm: objc2::MainThreadMarker,
) -> Retained<NSPopUpButton> {
    let popup = NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(mtm), frame, false);
    for label in labels {
        popup.addItemWithTitle(&NSString::from_str(label));
    }
    popup.selectItemAtIndex(selected as isize);
    popup
}
fn button(
    title: &str,
    help: &str,
    tag: isize,
    frame: NSRect,
    target: &Actions,
    color: bool,
    mtm: objc2::MainThreadMarker,
) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    button.setTitle(&NSString::from_str(title));
    button.setToolTip(Some(&NSString::from_str(help)));
    button.setTag(tag);
    button.setButtonType(NSButtonType::MomentaryChange);
    if color {
        button.setBordered(false);
    } else {
        button.setBezelStyle(NSBezelStyle::AccessoryBarAction);
    }
    unsafe {
        button.setTarget(Some(target));
        button.setAction(Some(if color {
            objc2::sel!(chooseColor:)
        } else {
            objc2::sel!(editRows:)
        }));
    }
    button
}
#[derive(Clone)]
struct Draft {
    target: Target,
    kind: Kind,
    text: String,
    color: u32,
}
impl From<&MarkRow> for Draft {
    fn from(row: &MarkRow) -> Self {
        Self {
            target: row.target,
            kind: row.kind,
            text: row.patterns.join(", "),
            color: row.marker_color(),
        }
    }
}

pub(super) fn open(browser: &mut Schnauz, cx: &mut Context<Schnauz>) {
    if browser.project_filter_editor {
        return;
    }
    browser.observe_project_library(cx);
    let project = browser.projects.active;
    let previous = browser.projects.sidebar().version_mark.clone();
    let name = project
        .and_then(|id| {
            browser
                .projects
                .items
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
        })
        .unwrap_or_else(|| "Personal".into());
    browser.project_filter_editor = true;
    browser.pending_click_rename = None;
    browser.rename = None;
    browser.context_menu = None;
    browser.sidebar_menu = None;
    browser.sort_menu = None;
    cx.notify();
    cx.spawn(async move|this,cx| {
        let mtm=objc2::MainThreadMarker::new().expect("Mark editor on main thread");
        let app=NSApplication::sharedApplication(mtm);
        let mut draft:Vec<Draft>=previous.rows.iter().map(Draft::from).collect();
        let mut error:Option<String>=None;
        let mut focus=0usize;
        loop {
            let alert=NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str(&format!("Mark rules — {name}")));
            let help="Choose Files or Folders for each rule. Newest version compares numeric suffixes (vX, vX.X); Version match also accepts v1 or v2.1. Name ending matches literal endings such as .pdf, _final.ai or _archive. Newest-version groups need two or more items; equal highest versions are all marked. Topmost matching rule supplies the color. All patterns ignore case. Mark On applies the saved rules.";
            alert.setInformativeText(&NSString::from_str(&error.as_ref().map(|e|format!("{e}\n\n{help}")).unwrap_or_else(||help.into())));
            let visible=(draft.len().clamp(1,6)*40) as f64;
            let height=(draft.len().max(1)*40) as f64;
            let content=NSView::initWithFrame(NSView::alloc(mtm),rect(0.,0.,760.,visible+26.));
            for (text,x,width) in [("Apply to",0.,92.),("Type",98.,142.),("Pattern(s)",248.,220.),("Marker color",480.,144.),("Priority",640.,90.)] {
                content.addSubview(&label(text,rect(x,visible+4.,width,20.),mtm));
            }
            let scroll=NSScrollView::initWithFrame(NSScrollView::alloc(mtm),rect(0.,0.,760.,visible));
            scroll.setHasVerticalScroller(true);scroll.setHasHorizontalScroller(false);scroll.setDrawsBackground(false);scroll.setAutohidesScrollers(true);
            let document=NSView::initWithFrame(NSView::alloc(mtm),rect(0.,0.,740.,height));
            let target=Actions::alloc(mtm).set_ivars(EditorState::default());
            let target:Retained<Actions>=unsafe{objc2::msg_send![super(target),init]};
            *target.ivars().colors.borrow_mut()=draft.iter().map(|d|d.color).collect();
            let mut controls=Vec::new();
            for (row,initial) in draft.iter().enumerate() {
                let y=height-(row+1) as f64*40.+6.;
                let scope=popup(&["Files","Folders"],if initial.target==Target::Files{0}else{1},rect(0.,y,92.,26.),mtm);
                let kinds=popup(&KINDS.map(Kind::label),KINDS.iter().position(|k|*k==initial.kind).unwrap_or(0),rect(98.,y,142.,26.),mtm);
                let field=project_file_filter::editor_field(&initial.text,mtm);field.setFrame(rect(248.,y,220.,26.));
                document.addSubview(&scope);document.addSubview(&kinds);document.addSubview(&field);
                let mut buttons=Vec::new();
                for (index,(name,color)) in PALETTE.into_iter().enumerate() {
                    let dot=button("",name,(row*6+index) as isize,rect(480.+index as f64*24.,y,24.,26.),&target,true,mtm);
                    dot.setImage(swatch(color,color==initial.color).as_deref());document.addSubview(&dot);buttons.push(dot);
                }
                target.ivars().swatches.borrow_mut().push(buttons);
                let up=button("↑","Move rule up · higher priority",3000+row as isize,rect(640.,y,24.,26.),&target,false,mtm);
                up.setEnabled(row>0);document.addSubview(&up);
                let down=button("↓","Move rule down · lower priority",4000+row as isize,rect(666.,y,24.,26.),&target,false,mtm);
                down.setEnabled(row+1<draft.len());document.addSubview(&down);
                document.addSubview(&button("−","Remove rule",2000+row as isize,rect(702.,y,24.,26.),&target,false,mtm));
                controls.push((scope,kinds,field));
            }
            if draft.is_empty(){document.addSubview(&label("No rules. Click Add Rule to begin.",rect(0.,8.,700.,24.),mtm));}
            scroll.setDocumentView(Some(&document));content.addSubview(&scroll);alert.setAccessoryView(Some(&content));
            alert.addButtonWithTitle(&NSString::from_str("Save"));
            alert.addButtonWithTitle(&NSString::from_str("Cancel")).setKeyEquivalent(&NSString::from_str("\u{1b}"));
            alert.addButtonWithTitle(&NSString::from_str("Add Rule")).setEnabled(draft.len()<MAX_RULES);
            let appearance=objc2_app_kit::NSAppearance::appearanceNamed(unsafe{if theme().is_light(){objc2_app_kit::NSAppearanceNameAqua}else{objc2_app_kit::NSAppearanceNameDarkAqua}});
            alert.window().setAppearance(appearance.as_deref());
            if let Some((_,_,field))=controls.get(focus.min(controls.len().saturating_sub(1))) {alert.window().setInitialFirstResponder(Some(field));}
            alert.layout();alert.window().center();
            let session=app.beginModalSessionForWindow(&alert.window());
            if let Some((_,_,field))=controls.get(focus.min(controls.len().saturating_sub(1))) {field.scrollRectToVisible(field.bounds());unsafe{field.selectText(None);}}
            let response=loop {
                let response=unsafe{app.runModalSession(session)};
                if response!=objc2_app_kit::NSModalResponseContinue{break response;}
                cx.background_executor().timer(Duration::from_millis(16)).await;
            };
            unsafe{app.endModalSession(session);}
            let colors=target.ivars().colors.borrow().clone();
            draft=controls.iter().enumerate().map(|(i,(scope,kind,field))|Draft{
                target:if scope.indexOfSelectedItem()==1{Target::Folders}else{Target::Files},
                kind:*KINDS.get(kind.indexOfSelectedItem().max(0) as usize).unwrap_or(&Kind::NewestVersion),
                text:field.stringValue().to_string(),color:colors.get(i).copied().unwrap_or(PALETTE[0].1)
            }).collect();
            alert.window().orderOut(None);
            if response==objc2_app_kit::NSAlertThirdButtonReturn && draft.len()<MAX_RULES {
                focus=draft.len();
                draft.push(if draft.is_empty(){Draft::from(&MarkRow::default())}else{Draft{target:Target::Files,kind:Kind::FilenameEnding,text:".pdf".into(),color:PALETTE[draft.len()%6].1}});
                error=None;continue;
            }
            if (2000..2000+draft.len() as isize).contains(&response){focus=(response-2000) as usize;draft.remove(focus);error=None;continue;}
            if (3000..3000+draft.len() as isize).contains(&response){let i=(response-3000) as usize;if i>0{draft.swap(i,i-1);focus=i-1;}error=None;continue;}
            if (4000..4000+draft.len() as isize).contains(&response){let i=(response-4000) as usize;if i+1<draft.len(){draft.swap(i,i+1);focus=i+1;}error=None;continue;}
            if response!=objc2_app_kit::NSAlertFirstButtonReturn{break;}
            if draft.len()>MAX_RULES {error=Some(format!("Use at most {MAX_RULES} rules."));continue;}
            let rows:Result<Vec<_>,_>=draft.iter().enumerate().map(|(i,d)|MarkRow::parse(d.target,d.kind,&d.text,d.color).map_err(|e|format!("Rule {}: {e}",i+1))).collect();
            match rows {
                Err(message)=>error=Some(message),
                Ok(rows)=>match this.update(cx,|this,cx|this.save_version_mark(project,&previous,Rule{enabled:previous.enabled,rows},cx)) {
                    Ok(Ok(()))=>break,Ok(Err(message))=>error=Some(message),Err(_)=>break,
                }
            }
        }
        let _=this.update(cx,|this,cx|{this.project_filter_editor=false;cx.notify();});
    }).detach();
}
