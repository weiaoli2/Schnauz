//! Todoist account routing and background requests. Tokens stay in the native
//! helper and Keychain; project links and caches contain no credentials.
use super::*;

#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
pub(super) struct Account {
    pub id: String,
    #[serde(rename = "userID")]
    pub user_id: String,
    pub name: String,
}
#[derive(serde::Deserialize)]
struct Config {
    version: u32,
    account: Option<Account>,
}
#[derive(Clone, Default)]
pub(super) struct Global {
    pub account: Option<Account>,
    pub busy: bool,
    pub notice: Option<String>,
    pub revision: u64,
    pub tasks_request: u64,
}
impl gpui::Global for Global {}
impl Global {
    pub fn load() -> Self {
        match read_account() {
            Ok(account) => Self {
                account,
                ..Self::default()
            },
            Err(error) => Self {
                notice: Some(error),
                ..Self::default()
            },
        }
    }
}
fn read_account() -> Result<Option<Account>, String> {
    let path = config_file("todoist.json").ok_or("Cannot find Todoist settings.")?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Cannot read Todoist settings. The file has been preserved.".into()),
    };
    if bytes.len() > 65536 {
        return Err("Todoist settings are too large.".into());
    }
    let config: Config = serde_json::from_slice(&bytes)
        .map_err(|_| "Todoist settings are invalid. The file has been preserved.")?;
    if config.version != 1
        || config
            .account
            .as_ref()
            .is_some_and(|a| a.id.is_empty() || a.user_id.is_empty())
    {
        return Err("Todoist settings are invalid. The file has been preserved.".into());
    }
    Ok(config.account)
}
#[derive(Clone, serde::Deserialize)]
pub(super) struct Project {
    pub id: String,
    pub name: String,
}
#[derive(Clone, serde::Deserialize)]
pub(super) struct Section {
    pub id: String,
    pub name: String,
    pub section_order: Option<i64>,
    pub order_key: Option<String>,
}
#[derive(Clone, serde::Deserialize)]
pub(super) struct Due {
    pub date: Option<String>,
    pub datetime: Option<String>,
    pub is_recurring: Option<bool>,
}
#[derive(Clone, serde::Deserialize)]
pub(super) struct TaskItem {
    pub id: String,
    pub project_id: String,
    pub content: String,
    pub updated_at: Option<String>,
    pub description: Option<String>,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub priority: Option<u8>,
    pub child_order: Option<i64>,
    pub order_key: Option<String>,
    pub due: Option<Due>,
}
#[derive(Clone, serde::Deserialize)]
pub(super) struct Snapshot {
    pub project: Project,
    pub tasks: Vec<TaskItem>,
    pub sections: Vec<Section>,
}
#[derive(Clone)]
pub(super) enum Row {
    Section(String),
    Task(usize, usize),
}
pub(super) fn outline(data: &Snapshot) -> Vec<Row> {
    fn walk(
        index: usize,
        depth: usize,
        data: &Snapshot,
        children: &HashMap<String, Vec<usize>>,
        seen: &mut HashSet<usize>,
        rows: &mut Vec<Row>,
    ) {
        if depth > 32 || !seen.insert(index) {
            return;
        }
        rows.push(Row::Task(index, depth));
        if let Some(items) = children.get(&data.tasks[index].id) {
            for &child in items {
                walk(child, depth + 1, data, children, seen, rows);
            }
        }
    }
    let mut order: Vec<usize> = (0..data.tasks.len()).collect();
    order.sort_by(|&a, &b| {
        data.tasks[a]
            .order_key
            .cmp(&data.tasks[b].order_key)
            .then(data.tasks[a].child_order.cmp(&data.tasks[b].child_order))
    });
    let ids: HashSet<&str> = data.tasks.iter().map(|t| t.id.as_str()).collect();
    let mut children: HashMap<String, Vec<usize>> = HashMap::new();
    for &i in &order {
        if let Some(parent) = &data.tasks[i].parent_id {
            children.entry(parent.clone()).or_default().push(i);
        }
    }
    let mut sections: Vec<&Section> = data.sections.iter().collect();
    sections.sort_by(|a, b| {
        a.order_key
            .cmp(&b.order_key)
            .then(a.section_order.cmp(&b.section_order))
    });
    let mut groups = vec![(None, "Tasks".to_string())];
    groups.extend(
        sections
            .iter()
            .map(|s| (Some(s.id.as_str()), s.name.clone())),
    );
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for (section, name) in groups {
        let roots: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| {
                let t = &data.tasks[i];
                t.section_id.as_deref().filter(|s| !s.is_empty()) == section
                    && t.parent_id.as_deref().is_none_or(|p| !ids.contains(p))
            })
            .collect();
        if !roots.is_empty() || section.is_some() {
            rows.push(Row::Section(name));
        }
        for i in roots {
            walk(i, 0, data, &children, &mut seen, &mut rows);
        }
    }
    // Keep orphaned tasks, unknown sections and malformed cycles visible once.
    for i in order {
        walk(i, 0, data, &children, &mut seen, &mut rows);
    }
    rows
}
#[derive(serde::Serialize)]
pub(super) struct Request {
    operation: String,
    config: PathBuf,
    account: Option<String>,
    project_id: Option<String>,
    project_name: Option<String>,
    task_id: Option<String>,
    request_id: Option<String>,
    expected_updated: Option<String>,
    allow_create: bool,
}
impl Request {
    fn account(operation: &str, account: Option<String>) -> Self {
        Self {
            operation: operation.into(),
            config: config_file("todoist.json").unwrap_or_default(),
            account,
            project_id: None,
            project_name: None,
            task_id: None,
            request_id: None,
            expected_updated: None,
            allow_create: false,
        }
    }
    fn linked(operation: &str, link: &projects::TodoistLink) -> Self {
        Self {
            project_id: Some(link.project_id.clone()),
            project_name: Some(link.project_name.clone()),
            request_id: Some(request_id()),
            ..Self::account(operation, Some(link.account_id.clone()))
        }
    }
}
#[derive(Default, serde::Deserialize)]
pub(super) struct Response {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub cancelled: bool,
    pub project: Option<Project>,
    pub snapshot: Option<Snapshot>,
    pub retry_after: Option<u64>,
}
pub(super) fn request_id() -> String {
    objc2_foundation::NSUUID::UUID().UUIDString().to_string()
}
fn run(request: Request) -> Response {
    let result = (|| -> Result<Response, ()> {
        use std::io::Write;
        let helper = std::env::current_exe()
            .map_err(|_| ())?
            .parent()
            .ok_or(())?
            .join("todoistctl");
        let input = serde_json::to_vec(&request).map_err(|_| ())?;
        let mut child = Command::new(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ())?;
        if !matches!(
            child.stdin.take().map(|mut stdin| stdin.write_all(&input)),
            Some(Ok(()))
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(());
        }
        let output = child.wait_with_output().map_err(|_| ())?;
        serde_json::from_slice(&output.stdout).map_err(|_| ())
    })();
    result.unwrap_or_else(|_| Response { message: "The Todoist helper is unavailable or stopped unexpectedly. Your existing tasks have been kept.".into(), ..Response::default() })
}
/// A UI request only: opening Tasks must not invalidate account/project caches.
pub(super) fn show_tasks(cx: &mut App) {
    let mut global = cx.global::<Global>().clone();
    global.tasks_request = global.tasks_request.wrapping_add(1);
    cx.set_global(global);
}

pub(super) fn account_action<T: 'static>(operation: &str, cx: &mut Context<T>) {
    let mut global = cx.global::<Global>().clone();
    if global.busy {
        return;
    }
    let request = Request::account(operation, global.account.as_ref().map(|a| a.id.clone()));
    let open_after_connect = operation == "connect";
    global.busy = true;
    global.notice = Some("Todoist: working…".into());
    cx.set_global(global);
    cx.spawn(async move |_, cx| {
        let response = cx.background_spawn(async move { run(request) }).await;
        let _ = cx.update(|cx| {
            let mut global = cx.global::<Global>().clone();
            global.busy = false;
            if response.success && !response.cancelled {
                match read_account() {
                    Ok(account) => {
                        global.account = account;
                        global.revision += 1;
                        if open_after_connect && global.account.is_some() {
                            global.tasks_request = global.tasks_request.wrapping_add(1);
                        }
                    }
                    Err(error) => {
                        global.notice = Some(error);
                        cx.set_global(global);
                        return;
                    }
                }
            }
            global.notice = Some(response.message);
            cx.set_global(global);
        });
    })
    .detach();
}

pub(super) type Key = (String, String);
fn set_backoff(state: &mut State, key: Key, delay: u64, message: String) {
    let cache = state.caches.entry(key).or_default();
    cache.retry_at = Some(Instant::now() + Duration::from_secs(delay.clamp(5, 3600)));
    cache.error = Some(message);
}
#[derive(Default)]
pub(super) struct Cache {
    pub data: Option<Snapshot>,
    pub rows: Vec<Row>,
    pub busy: bool,
    pub error: Option<String>,
    pub good_at: Option<Instant>,
    pub retry_at: Option<Instant>,
    pub refresh_after: bool,
}
impl Cache {
    /// A write may finish while an older read is in flight. Discard that read
    /// and request a fresh snapshot rather than making old tasks look current.
    fn finish_read(&mut self, response: Response, project_id: &str) -> bool {
        self.busy = false;
        if std::mem::take(&mut self.refresh_after) {
            self.good_at = None;
            if response.success {
                return true;
            }
        }
        if response.success {
            if let Some(data) = response.snapshot.filter(|s| {
                s.project.id == project_id && s.tasks.iter().all(|t| t.project_id == project_id)
            }) {
                self.rows = outline(&data);
                self.data = Some(data);
                self.good_at = Some(Instant::now());
                self.error = None;
                self.retry_at = None;
            } else {
                self.error =
                    Some("Todoist returned the wrong project. Cached tasks were kept.".into());
                self.retry_at = Some(Instant::now() + Duration::from_secs(30));
            }
        } else {
            self.error = Some(response.message);
            self.retry_at = Some(
                Instant::now()
                    + Duration::from_secs(response.retry_after.unwrap_or(30).clamp(5, 3600)),
            );
        }
        false
    }
}
#[derive(Default)]
pub(super) struct State {
    pub open: bool,
    pub caches: HashMap<Key, Cache>,
    pub pending: HashSet<String>,
    pub attempted: HashSet<(Option<u64>, String, String)>,
    pub link_errors: HashMap<Option<u64>, String>,
    pub completions: HashMap<(String, String, String), (Option<String>, String)>,
    pub revision: u64,
    pub tasks_request: u64,
    pub ticker: bool,
    pub scroll: UniformListScrollHandle,
}
impl State {
    /// A new account invalidates data; a Show tasks request only opens the panel.
    pub(super) fn observe_account(&mut self, global: &Global) -> (bool, bool) {
        let changed = self.revision != global.revision;
        let requested = self.tasks_request != global.tasks_request && global.account.is_some();
        self.tasks_request = global.tasks_request;
        if changed {
            self.caches.clear();
            self.attempted.clear();
            self.pending.clear();
            self.link_errors.clear();
            self.completions.clear();
            self.revision = global.revision;
        }
        (changed, requested)
    }

    fn completion_request(&mut self, link: &projects::TodoistLink, task: String) -> Request {
        let key = (link.account_id.clone(), link.project_id.clone());
        let updated = self
            .caches
            .get(&key)
            .and_then(|c| c.data.as_ref())
            .and_then(|s| s.tasks.iter().find(|t| t.id == task))
            .and_then(|t| t.updated_at.clone());
        let intent = self
            .completions
            .entry((key.0, key.1, task.clone()))
            .or_insert_with(|| (updated.clone(), request_id()));
        if intent.0 != updated {
            *intent = (updated.clone(), request_id());
        }
        let mut request = Request::linked("complete", link);
        request.task_id = Some(task);
        request.expected_updated = updated;
        request.request_id = Some(intent.1.clone());
        request
    }
}
impl Schnauz {
    pub(super) fn todoist_link(&self, cx: &App) -> Option<projects::TodoistLink> {
        let link = self.projects.sidebar().todoist.as_ref()?;
        let account = cx.global::<Global>().account.as_ref()?;
        (link.account_id == account.id).then(|| {
            let mut link = link.clone();
            if let Some(data) = self
                .todoist
                .caches
                .get(&(link.account_id.clone(), link.project_id.clone()))
                .and_then(|c| c.data.as_ref())
            {
                link.project_name = data.project.name.clone();
            }
            link
        })
    }
    pub(super) fn ensure_todoist(&mut self, force: bool, cx: &mut Context<Self>) {
        if !self.todoist.open || cx.global::<Global>().busy {
            return;
        }
        let Some(account) = cx.global::<Global>().account.as_ref().cloned() else {
            return;
        };
        let Some(link) = self.todoist_link(cx) else {
            self.link_todoist(false, cx);
            return;
        };
        let key = (account.id.clone(), link.project_id.clone());
        let entry = self.todoist.caches.entry(key.clone()).or_default();
        if entry.busy
            || entry.retry_at.is_some_and(|t| t > Instant::now())
            || (!force
                && entry
                    .good_at
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(30)))
        {
            return;
        }
        entry.busy = true;
        let revision = cx.global::<Global>().revision;
        let request = Request::linked("snapshot", &link);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let response = cx.background_spawn(async move { run(request) }).await;
            let _ = this.update(cx, |this, cx| {
                if cx.global::<Global>().revision != revision {
                    return;
                }
                let cache = this.todoist.caches.entry(key).or_default();
                if cache.finish_read(response, &link.project_id) {
                    this.ensure_todoist(true, cx);
                } else if let Some(remote_name) =
                    cache.data.as_ref().map(|d| d.project.name.clone())
                {
                    let mut candidate = this.projects.clone();
                    let ids: Vec<u64> = candidate
                        .items
                        .iter()
                        .filter(|p| {
                            p.sidebar.todoist.as_ref().is_some_and(|l| {
                                l.account_id == link.account_id && l.project_id == link.project_id
                            }) && p.name != remote_name
                        })
                        .map(|p| p.id)
                        .collect();
                    if !ids.is_empty() {
                        let result = ids
                            .into_iter()
                            .try_for_each(|id| candidate.set_name(id, &remote_name))
                            .and_then(|_| this.persist_library(candidate));
                        if let Err(error) = result {
                            this.set_notice(
                                format!("Todoist name could not be matched: {error}"),
                                cx,
                            );
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn link_todoist(&mut self, create: bool, cx: &mut Context<Self>) {
        let Some(account) = cx.global::<Global>().account.as_ref().cloned() else {
            return;
        };
        let origin = self.projects.active;
        let name = origin
            .and_then(|id| self.projects.items.iter().find(|p| p.id == id))
            .map_or_else(|| "Personal".into(), |p| p.name.clone());
        let attempt = (origin, account.id.clone(), name.clone());
        if !create && !self.todoist.attempted.insert(attempt.clone()) {
            return;
        }
        let busy = format!("link:{origin:?}");
        if !self.todoist.pending.insert(busy.clone()) {
            return;
        }
        self.todoist.link_errors.remove(&origin);
        let mut candidate = self.projects.clone();
        let sidebar = candidate.sidebar_mut();
        if create && sidebar.todoist_create_request.is_none() {
            sidebar.todoist_create_request = Some(request_id());
        }
        let intent = sidebar.todoist_create_request.clone();
        if create {
            if let Err(error) = self.persist_library(candidate) {
                self.todoist.pending.remove(&busy);
                self.set_notice(error, cx);
                return;
            }
        }
        let mut request = Request::account("ensure-project", Some(account.id.clone()));
        request.project_name = Some(name.clone());
        request.allow_create = intent.is_some();
        request.request_id = intent.clone();
        let revision = cx.global::<Global>().revision;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let response = cx.background_spawn(async move { run(request) }).await;
            let _ = this.update(cx, |this, cx| {
                if cx.global::<Global>().revision != revision {
                    return;
                }
                this.todoist.pending.remove(&busy);
                let mut candidate = this.projects.clone();
                let current_name = origin
                    .and_then(|id| candidate.items.iter().find(|p| p.id == id))
                    .map_or_else(|| "Personal".into(), |p| p.name.clone());
                let Some(sidebar) = candidate.sidebar_for(origin) else {
                    return;
                };
                if current_name != name || sidebar.todoist_create_request != intent {
                    return;
                }
                if response.success {
                    if let Some(project) = response.project {
                        if let Some(id) = origin {
                            if let Err(error) = candidate.set_name(id, &project.name) {
                                this.todoist.link_errors.insert(origin, error.clone());
                                this.set_notice(error, cx);
                                return;
                            }
                        }
                        let sidebar = candidate.sidebar_for_mut(origin).unwrap();
                        sidebar.todoist = Some(projects::TodoistLink {
                            account_id: account.id,
                            project_id: project.id,
                            project_name: project.name,
                        });
                        sidebar.todoist_create_request = None;
                        if let Err(error) = this.persist_library(candidate) {
                            this.set_notice(error, cx);
                        } else if this.projects.active == origin {
                            this.ensure_todoist(true, cx);
                        }
                    } else {
                        this.todoist.link_errors.insert(origin, response.message);
                    }
                } else if !response.cancelled {
                    this.todoist
                        .link_errors
                        .insert(origin, response.message.clone());
                    this.set_notice(response.message, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn todoist_mutation(
        &mut self,
        operation: &str,
        task: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(link) = self.todoist_link(cx) else {
            return;
        };
        let key = (link.account_id.clone(), link.project_id.clone());
        if self
            .todoist
            .caches
            .get(&key)
            .is_some_and(|c| c.retry_at.is_some_and(|t| t > Instant::now()))
        {
            self.set_notice("Wait briefly before the next Todoist request.".into(), cx);
            return;
        }
        let busy = task.as_ref().map_or_else(
            || format!("add:{}", link.project_id),
            |id| format!("complete:{id}"),
        );
        if !self.todoist.pending.insert(busy.clone()) {
            return;
        }
        let completion = task.as_ref().map(|task| {
            (
                link.account_id.clone(),
                link.project_id.clone(),
                task.clone(),
            )
        });
        let request = if let Some(task) = task {
            self.todoist.completion_request(&link, task)
        } else {
            Request::linked(operation, &link)
        };
        let revision = cx.global::<Global>().revision;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let response = cx.background_spawn(async move { run(request) }).await;
            let _ = this.update(cx, |this, cx| {
                if cx.global::<Global>().revision != revision {
                    return;
                }
                this.todoist.pending.remove(&busy);
                if response.success && !response.cancelled {
                    if let Some(completion) = completion {
                        this.todoist.completions.remove(&completion);
                    }
                    let cache = this.todoist.caches.entry(key).or_default();
                    cache.refresh_after = cache.busy;
                    cache.good_at = None;
                    cache.retry_at = None;
                    this.ensure_todoist(true, cx);
                    this.set_notice(response.message, cx);
                } else if !response.cancelled {
                    let cache = this.todoist.caches.entry(key.clone()).or_default();
                    cache.good_at = None;
                    cache.refresh_after = cache.busy;
                    if let Some(delay) = response.retry_after {
                        self::set_backoff(&mut this.todoist, key, delay, response.message.clone());
                    }
                    this.ensure_todoist(true, cx);
                    this.set_notice(response.message, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn open_todoist(&mut self, cx: &mut Context<Self>) {
        self.dismiss_space_map |= self.space_map.is_some();
        self.todoist.open = true;
        self.todoist.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.ensure_todoist(false, cx);
        if !self.todoist.ticker {
            self.todoist.ticker = true;
            cx.spawn(async move |this, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_secs(30))
                    .await;
                let alive = this
                    .update(cx, |this, cx| {
                        if !this.todoist.open {
                            this.todoist.ticker = false;
                            return false;
                        }
                        let active = objc2::MainThreadMarker::new().is_some_and(|m| {
                            objc2_app_kit::NSApplication::sharedApplication(m).isActive()
                        });
                        if active {
                            this.ensure_todoist(false, cx);
                        }
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            })
            .detach();
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(project: &str, tasks: serde_json::Value) -> Snapshot {
        serde_json::from_value(serde_json::json!({"project":{"id":project,"name":"Work"},"tasks":tasks,"sections":[{"id":"section-A","name":"Section","section_order":1}]})).unwrap()
    }
    #[test]
    fn delayed_reads_cannot_replace_post_write_state_and_failed_reads_keep_cached_tasks() {
        let old = snapshot(
            "A",
            serde_json::json!([{"id":"task-A","project_id":"A","content":"Task"}]),
        );
        let mut cache = Cache {
            data: Some(old.clone()),
            rows: outline(&old),
            busy: true,
            refresh_after: true,
            ..Cache::default()
        };
        assert!(cache.finish_read(
            Response {
                success: true,
                snapshot: Some(old),
                ..Response::default()
            },
            "A"
        ));
        assert!(!cache.busy && cache.good_at.is_none());
        let fresh = snapshot("A", serde_json::json!([]));
        assert!(!cache.finish_read(
            Response {
                success: true,
                snapshot: Some(fresh),
                ..Response::default()
            },
            "A"
        ));
        assert!(cache.data.as_ref().unwrap().tasks.is_empty());
        assert!(cache.good_at.is_some());
        cache.finish_read(
            Response {
                message: "Rate limited".into(),
                retry_after: Some(60),
                ..Response::default()
            },
            "A",
        );
        assert!(cache.retry_at.unwrap() > Instant::now() + Duration::from_secs(58));
        assert_eq!(cache.data.as_ref().unwrap().project.id, "A");
        let wrong = snapshot("B", serde_json::json!([]));
        cache.finish_read(
            Response {
                success: true,
                snapshot: Some(wrong),
                ..Response::default()
            },
            "A",
        );
        assert_eq!(cache.data.as_ref().unwrap().project.id, "A");
        assert!(cache.error.as_ref().unwrap().contains("wrong project"));
    }
    #[test]
    fn outline_preserves_subtasks_orphans_and_cycles_once() {
        let data = snapshot(
            "A",
            serde_json::json!([
                {"id":"child","project_id":"A","content":"Child","parent_id":"parent","section_id":"section-A","child_order":2},
                {"id":"parent","project_id":"A","content":"Parent","section_id":"section-A","child_order":1},
                {"id":"cycle-1","project_id":"A","content":"Cycle 1","parent_id":"cycle-2"},
                {"id":"cycle-2","project_id":"A","content":"Cycle 2","parent_id":"cycle-1"},
                {"id":"orphan","project_id":"A","content":"Orphan","parent_id":"missing"},
                {"id":"unknown-section","project_id":"A","content":"Unknown section","section_id":"missing-section"}
            ]),
        );
        let rows = outline(&data);
        let tasks: Vec<(&str, usize)> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Task(i, depth) => Some((data.tasks[*i].id.as_str(), *depth)),
                _ => None,
            })
            .collect();
        assert_eq!(tasks.len(), data.tasks.len());
        assert_eq!(
            tasks.iter().map(|t| t.0).collect::<HashSet<_>>().len(),
            data.tasks.len()
        );
        assert!(tasks.contains(&("parent", 0)) && tasks.contains(&("child", 1)));
        assert!(tasks.contains(&("orphan", 0)));
    }
    #[test]
    fn opening_tasks_preserves_cache_but_account_changes_discard_it() {
        let mut state = State::default();
        let key = ("account".to_string(), "A".to_string());
        state.caches.insert(key.clone(), Cache { data: Some(snapshot("A", serde_json::json!([]))), ..Cache::default() });
        let mut global = Global {
            account: Some(Account { id: "account".into(), user_id: "user".into(), name: "Test".into() }),
            tasks_request: 1,
            ..Global::default()
        };
        assert_eq!(state.observe_account(&global), (false, true));
        assert!(state.caches[&key].data.is_some());
        assert_eq!(state.observe_account(&global), (false, false));
        global.revision += 1;
        global.account = None;
        assert_eq!(state.observe_account(&global), (true, false));
        assert!(state.caches.is_empty());
    }

    #[test]
    fn uncertain_completions_reuse_the_request_until_the_visible_task_revision_changes() {
        let link = projects::TodoistLink {
            account_id: "account".into(),
            project_id: "A".into(),
            project_name: "Work".into(),
        };
        let mut state = State::default();
        state.caches.insert(("account".into(),"A".into()), Cache { data:Some(snapshot("A",serde_json::json!([{"id":"recurring","project_id":"A","content":"Daily","updated_at":"2026-10-17T10:00:00Z"}]))), ..Cache::default() });
        let first = state.completion_request(&link, "recurring".into());
        let retry = state.completion_request(&link, "recurring".into());
        assert_eq!(first.request_id, retry.request_id);
        assert_eq!(
            first.expected_updated.as_deref(),
            Some("2026-10-17T10:00:00Z")
        );
        state
            .caches
            .get_mut(&("account".into(), "A".into()))
            .unwrap()
            .data
            .as_mut()
            .unwrap()
            .tasks[0]
            .updated_at = Some("2026-10-18T10:00:00Z".into());
        let next_occurrence = state.completion_request(&link, "recurring".into());
        assert_ne!(first.request_id, next_occurrence.request_id);
        assert_eq!(next_occurrence.project_id.as_deref(), Some("A"));
    }
}
