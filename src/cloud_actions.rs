//! Provider-aware menus. Only expose operations Schnauz can actually perform.
use super::*;

impl Schnauz {
    pub(super) fn cloud_menu(
        &self,
        pane: usize,
        path: &Path,
        is_dir: bool,
        cx: &Context<Self>,
    ) -> Option<MenuNode> {
        let kind = cloud_kind(path);
        let linked = nextcloud::location(path, cx);
        if kind.is_none() && linked.is_none() {
            return None;
        }
        let provider = linked
            .or_else(|| cloud_locations::identify(&home_dir(), path))
            .map(|c| c.name)
            .unwrap_or_else(|| cloud_provider_name(path));
        let mut items = vec![MenuNode::Disabled(
            cloud_storage_label(self.cloud_state(path, is_dir, cx)).into(),
        )];
        let server_items = nextcloud::menu_items(path, is_dir, cx);
        if !server_items.is_empty() {
            push_menu_sep(&mut items);
            items.extend(server_items);
        }
        if provider == "Nextcloud" && kind == Some(CloudKind::Provider) {
            if let Some(error) = nextcloud_local::error(path) {
                push_menu_sep(&mut items);
                items.push(MenuNode::Disabled(error));
            }
            if nextcloud_local::read(path).is_some_and(|item| item.can_keep())
                && !nextcloud_local::pin_busy(path)
            {
                push_menu_sep(&mut items);
                let path = path.to_path_buf();
                items.push(mi("Always keep downloaded", move |this, _, cx| {
                    this.close_context_menu(cx);
                    nextcloud::launch("keep-downloaded", None, Some(path.clone()), cx);
                }));
            }
        }
        if kind.is_some() && cloudctl_path().is_some() {
            push_menu_sep(&mut items);
            if is_dir && provider != "Nextcloud" {
                let path = path.to_path_buf();
                items.push(mi("Sync Now", move |this, _, cx| {
                    this.cloud_sync(pane, path.clone(), cx)
                }));
            }
            // Never traverse a folder just to construct a context menu.
            if provider != "Nextcloud" && (is_dir || cached_dataless(path)) {
                let path = path.to_path_buf();
                items.push(mi(
                    if is_dir {
                        "Download Contents"
                    } else {
                        "Download Now"
                    },
                    move |this, _, cx| this.cloud_download(pane, path.clone(), cx),
                ));
            }
            if kind == Some(CloudKind::ICloud) && (is_dir || !cached_dataless(path)) {
                let path = path.to_path_buf();
                items.push(mi(
                    if is_dir {
                        "Free Up Space in Folder"
                    } else {
                        "Free Up Space"
                    },
                    move |this, _, cx| this.cloud_evict(pane, path.clone(), cx),
                ));
            }
        }
        if provider != "Nextcloud" {
            push_menu_sep(&mut items);
            let path = path.to_path_buf();
            items.push(mi(
                format!("{provider} actions in Finder…"),
                move |this, _, cx| {
                    this.close_context_menu(cx);
                    let _ = Command::new("open").arg("-R").arg(&path).spawn();
                },
            ));
        }
        while matches!(items.last(), Some(MenuNode::Sep)) {
            items.pop();
        }
        Some(MenuNode::Sub {
            label: provider,
            items,
        })
    }

    pub(super) fn start_cloud_file_operation(
        &mut self,
        pane: usize,
        path: PathBuf,
        download: bool,
        cx: &mut Context<Self>,
    ) {
        self.close_context_menu(cx);
        let Some(tool) = cloudctl_path() else {
            return;
        };
        if !self.cloud_busy.insert(path.clone()) {
            return;
        }
        let directory = self.tab(pane).current_dir.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let scan = path.clone();
            let targets = cx
                .background_spawn(async move {
                    collect_cloud_files(&scan, download, if download { 2000 } else { 5000 })
                })
                .await;
            let working = targets.clone();
            if this
                .update(cx, |this, cx| {
                    this.cloud_busy.extend(working);
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            let files = targets.clone();
            let error = cx
                .background_spawn(async move {
                    let mut error = None;
                    for file in files {
                        match Command::new(&tool)
                            .arg(if download { "download" } else { "evict" })
                            .arg(file)
                            .output()
                        {
                            Ok(output) if output.status.success() => {}
                            Ok(output) => {
                                error =
                                    Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
                            }
                            Err(failure) => error = Some(failure.to_string()),
                        }
                    }
                    error
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                cloud_status::invalidate(&path);
                for file in &targets {
                    cloud_status::invalidate(file);
                }
                this.cloud_busy.remove(&path);
                for file in &targets {
                    this.cloud_busy.remove(file);
                }
                if let Some(error) = error {
                    this.set_notice(format!("Cloud operation failed: {error}"), cx);
                }
                if pane < this.panes.len()
                    && this.tab(pane).remote.is_none()
                    && this.tab(pane).current_dir == directory
                {
                    this.refresh_pane(pane, cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
