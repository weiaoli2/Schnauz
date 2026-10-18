//! Installed Automator file actions, alongside Schnauz's built-in image actions.
use super::*;
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSArray, NSDictionary};

type Dict = NSDictionary<AnyObject, AnyObject>;
#[link(name = "UniformTypeIdentifiers", kind = "framework")]
unsafe extern "C" {}

#[derive(Clone, Debug)]
struct Workflow {
    name: String,
    path: PathBuf,
    types: Vec<String>,
}
fn value(dict: &Dict, key: &str) -> Option<Retained<AnyObject>> {
    dict.objectForKey(&NSString::from_str(key))
}
fn string(dict: &Dict, key: &str) -> Option<String> {
    Some(value(dict, key)?.downcast_ref::<NSString>()?.to_string())
}
fn read_workflow(path: &Path) -> Vec<Workflow> {
    let Some(file) = path
        .join("Contents/Info.plist")
        .to_str()
        .map(NSString::from_str)
    else {
        return Vec::new();
    };
    #[allow(deprecated)]
    let Some(info) = (unsafe { Dict::dictionaryWithContentsOfFile(&file) }) else {
        return Vec::new();
    };
    let Some(services) = value(&info, "NSServices") else {
        return Vec::new();
    };
    let Some(services) = services.downcast_ref::<NSArray<AnyObject>>() else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| {
            let service = service.downcast_ref::<Dict>()?;
            if string(service, "NSMessage").as_deref() != Some("runWorkflowAsService") {
                return None;
            }
            let menu = value(service, "NSMenuItem")?;
            let name = string(menu.downcast_ref::<Dict>()?, "default")?;
            // Finder-targeted file actions are useful in Schnauz too. Do not import
            // services restricted to unrelated editors or application-specific input.
            if let Some(context) = value(service, "NSRequiredContext") {
                if let Some(context) = context.downcast_ref::<Dict>() {
                    if let Some(app) = string(context, "NSApplicationIdentifier") {
                        if !matches!(app.as_str(), "com.apple.finder" | "local.schnauz.app") {
                            return None;
                        }
                    }
                }
            }
            let types = value(service, "NSSendFileTypes")?;
            let types = types
                .downcast_ref::<NSArray<AnyObject>>()?
                .iter()
                .filter_map(|v| v.downcast_ref::<NSString>().map(|v| v.to_string()))
                .collect::<Vec<_>>();
            if types.is_empty() {
                return None;
            }
            Some(Workflow {
                name,
                path: path.into(),
                types,
            })
        })
        .collect()
}
fn installed() -> Vec<Workflow> {
    let mut roots = vec![PathBuf::from("/Library/Services")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.insert(0, PathBuf::from(home).join("Library/Services"));
    }
    let mut workflows = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("workflow"))
            {
                workflows.extend(read_workflow(&path));
            }
        }
    }
    workflows.sort_by_key(|w| w.name.to_lowercase());
    workflows.dedup_by(|a, b| a.name == b.name);
    workflows
}
fn accepts(workflow: &Workflow, path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    unsafe {
        let actual: Option<Retained<AnyObject>> = if path.is_dir() {
            msg_send![class!(UTType), typeWithIdentifier: &*NSString::from_str("public.folder")]
        } else {
            msg_send![class!(UTType), typeWithFilenameExtension: &*NSString::from_str(&extension)]
        };
        let Some(actual) = actual else {
            return false;
        };
        workflow.types.iter().any(|identifier| {
            let accepted: Option<Retained<AnyObject>> =
                msg_send![class!(UTType), typeWithIdentifier: &*NSString::from_str(identifier)];
            accepted.is_some_and(|accepted| {
                let conforms: bool = msg_send![&*actual, conformsToType: &*accepted];
                conforms
            })
        })
    }
}
fn command(workflow: &Workflow, path: &Path) -> Command {
    let mut command = Command::new("/usr/bin/automator");
    // Arguments remain separate even for quotes, newlines and shell metacharacters.
    command.arg("-i").arg(path).arg(&workflow.path);
    command
}
pub(super) fn nodes(path: &Path) -> Vec<MenuNode> {
    installed()
        .into_iter()
        .filter(|workflow| accepts(workflow, path))
        .map(|workflow| workflow_node(workflow, path))
        .collect()
}
fn workflow_node(workflow: Workflow, path: &Path) -> MenuNode {
    let path = path.to_path_buf();
    mi(workflow.name.clone(), move |this, _, cx| {
        this.close_context_menu(cx);
        let workflow = workflow.clone();
        let path = path.clone();
        let name = workflow.name.clone();
        this.set_notice(format!("Running {name}…"), cx);
        cx.spawn(async move |this, cx| {
            let success = cx
                .background_spawn(async move {
                    command(&workflow, &path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .is_ok_and(|status| status.success())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.set_notice(
                    if success {
                        format!("{name} finished")
                    } else {
                        format!("{name} failed. Check the workflow in Automator.")
                    },
                    cx,
                );
            });
        })
        .detach();
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_conflicts_and_required_modifiers_are_checked() {
        let mut keymap = Keymap::defaults();
        assert!(validate_binding("a", "m", &keymap).is_err());
        assert!(validate_binding("a", "cmd-c", &keymap).is_err());
        assert!(validate_binding("a", "cmd-q", &keymap).is_err());
        assert!(validate_binding("a", "cmd-alt-m", &keymap).is_ok());
        keymap.quick.insert("a".into(), "cmd-alt-m".into());
        assert!(validate_binding("a", "cmd-alt-m", &keymap).is_ok());
        assert!(validate_binding("b", "cmd-alt-m", &keymap).is_err());
    }
    #[test]
    fn automator_receives_the_selected_file_without_shell_interpolation() {
        let root =
            std::env::temp_dir().join(format!("schnauz-workflow-run-{}", std::process::id()));
        let bundle = root.join("Fixture.workflow");
        fs::create_dir_all(bundle.join("Contents")).unwrap();
        let document = r#"<?xml version="1.0"?><plist version="1.0"><dict>
        <key>AMDocumentVersion</key><string>2</string>
        <key>actions</key><array><dict><key>action</key><dict>
        <key>AMAccepts</key><dict><key>Container</key><string>List</string><key>Optional</key><true/><key>Types</key><array><string>com.apple.cocoa.string</string></array></dict>
        <key>AMProvides</key><dict><key>Container</key><string>List</string><key>Types</key><array><string>com.apple.cocoa.string</string></array></dict>
        <key>ActionBundlePath</key><string>/System/Library/Automator/Run Shell Script.action</string>
        <key>BundleIdentifier</key><string>com.apple.RunShellScript</string>
        <key>Class Name</key><string>RunShellScriptAction</string>
        <key>ActionParameters</key><dict><key>COMMAND_STRING</key><string>printf '%s' "$1" &gt; "$1.received"</string><key>inputMethod</key><integer>1</integer><key>shell</key><string>/bin/zsh</string><key>source</key><string></string></dict>
        </dict></dict></array>
        <key>workflowMetaData</key><dict><key>workflowTypeIdentifier</key><string>com.apple.Automator.servicesMenu</string></dict>
        </dict></plist>"#;
        fs::write(bundle.join("Contents/document.wflow"), document).unwrap();
        let path = root.join("PNG 'quotes' $(never executed).png");
        fs::write(&path, b"fixture").unwrap();
        let action = Workflow {
            name: "Fixture".into(),
            path: bundle,
            types: vec!["public.png".into()],
        };
        let output = command(&action, &path).output().unwrap();
        assert!(
            output.status.success(),
            "Disposable Automator fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(path.with_file_name(format!(
                "{}.received",
                path.file_name().unwrap().to_str().unwrap()
            )))
            .unwrap(),
            path.to_str().unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn installed_file_actions_match_native_image_types_and_keep_arguments_literal() {
        let root = std::env::temp_dir().join(format!("schnauz-actions-{}", std::process::id()));
        let workflow = root.join("Measure Render.workflow");
        fs::create_dir_all(workflow.join("Contents")).unwrap();
        fs::write(workflow.join("Contents/Info.plist"), r#"<?xml version="1.0"?><plist version="1.0"><dict><key>NSServices</key><array><dict><key>NSMessage</key><string>runWorkflowAsService</string><key>NSMenuItem</key><dict><key>default</key><string>Measure Render</string></dict><key>NSRequiredContext</key><dict><key>NSApplicationIdentifier</key><string>com.apple.finder</string></dict><key>NSSendFileTypes</key><array><string>public.png</string></array></dict></array></dict></plist>"#).unwrap();
        let actions = read_workflow(&workflow);
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(action.name, "Measure Render");
        assert!(accepts(action, Path::new("image.PNG")));
        assert!(!accepts(action, Path::new("image.jpg")));
        assert!(!accepts(action, &root));
        let mut all_images = action.clone();
        all_images.types = vec!["public.image".into()];
        assert!(accepts(&all_images, Path::new("image.png")));
        assert!(accepts(&all_images, Path::new("image.jpeg")));
        let path = root.join("'quoted' $(never)\nimage.png");
        assert_eq!(
            command(action, &path).get_args().collect::<Vec<_>>(),
            vec![
                std::ffi::OsStr::new("-i"),
                path.as_os_str(),
                workflow.as_os_str()
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }
}

const BUILTINS: &[(&str, &str)] = &[
    ("rotate_left", "Rotate Left"),
    ("rotate_right", "Rotate Right"),
    ("markup", "Markup"),
    ("create_pdf", "Create PDF"),
    ("jpeg", "Convert to JPEG"),
    ("png", "Convert to PNG"),
    ("heic", "Convert to HEIC"),
    ("remove_background", "Remove Background"),
];
fn workflow_id(path: &Path) -> String {
    format!("workflow:{}", path.to_string_lossy())
}
pub(super) fn assignable() -> Vec<(String, String)> {
    BUILTINS
        .iter()
        .map(|(id, name)| (format!("builtin:{id}"), (*name).into()))
        .chain(
            installed()
                .into_iter()
                .map(|w| (workflow_id(&w.path), w.name)),
        )
        .collect()
}
pub(super) fn validate_binding(id: &str, binding: &str, keymap: &Keymap) -> Result<(), String> {
    if !["cmd-", "ctrl-", "alt-"]
        .iter()
        .any(|modifier| binding.contains(modifier))
    {
        return Err("Include Command, Control or Option in a Quick Action shortcut.".into());
    }
    if let Some(action) = keymap.action_for(binding) {
        return Err(format!("Already assigned to {}.", action.label()));
    }
    if matches!(
        binding,
        "cmd-q"
            | "cmd-h"
            | "cmd-alt-h"
            | "cmd-m"
            | "cmd-,"
            | "cmd-l"
            | "cmd-k"
            | "cmd-shift-s"
            | "cmd-x"
            | "cmd-shift-z"
            | "cmd-shift-d"
            | "cmd-shift-i"
    ) {
        return Err("This shortcut is reserved by Schnauz or macOS.".into());
    }
    if keymap
        .quick
        .iter()
        .any(|(other, key)| other != id && key == binding)
    {
        return Err("Already assigned to another Quick Action.".into());
    }
    Ok(())
}
impl Schnauz {
    pub(super) fn open_quick_actions(
        &mut self,
        pane: usize,
        x: f32,
        y: f32,
        path: PathBuf,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        self.active_pane = pane;
        self.pending_click_rename = None;
        self.rename = None;
        let nodes = if self.tab(pane).remote.is_some() {
            vec![MenuNode::Disabled(
                "Quick Actions require a local file or folder".into(),
            )]
        } else {
            self.quick_actions_nodes(pane, &path)
        };
        self.context_menu = Some(ContextMenu {
            x,
            y,
            target: Some((path, is_dir)),
            nodes,
        });
        cx.notify();
    }
    pub(super) fn run_quick_action_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let keymap = keymap();
        let Some((id, _)) = keymap
            .quick
            .iter()
            .find(|(_, binding)| binding.as_str() == key)
        else {
            return false;
        };
        // Never steal a normal configured binding if it was subsequently changed.
        if keymap.action_for(key).is_some() {
            return false;
        }
        let pane = self.active_pane;
        if self.tab(pane).remote.is_some() {
            self.set_notice("Quick Actions require a local file or folder".into(), cx);
            return true;
        }
        let targets = self.action_targets(pane);
        if targets.len() != 1 {
            self.set_notice("Select one file or folder for this Quick Action".into(), cx);
            return true;
        }
        let path = &targets[0];
        let action = if !path.exists() {
            None
        } else if let Some(builtin) = id.strip_prefix("builtin:") {
            let label = BUILTINS
                .iter()
                .find(|(id, _)| *id == builtin)
                .map(|(_, label)| *label);
            self.quick_actions_nodes(pane, path)
                .into_iter()
                .find_map(|node| match node {
                    MenuNode::Item {
                        label: name, on, ..
                    } if Some(name.as_str()) == label => Some(on),
                    _ => None,
                })
        } else {
            installed()
                .into_iter()
                .find(|workflow| workflow_id(&workflow.path) == *id && accepts(workflow, path))
                .and_then(|workflow| match workflow_node(workflow, path) {
                    MenuNode::Item { on, .. } => Some(on),
                    _ => None,
                })
        };
        if let Some(action) = action {
            action(self, window, cx);
        } else {
            self.set_notice(
                "This Quick Action is unavailable for the selected item".into(),
                cx,
            );
        }
        true
    }
}
