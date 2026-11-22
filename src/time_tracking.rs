//! Project accounting. Runtime ownership is never inferred from the selected tab.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub(super) fn key(project: Option<u64>) -> String {
    project
        .map(|id| id.to_string())
        .unwrap_or_else(|| "personal".into())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(super) struct Totals {
    pub personal_ms: u64,
    pub codex_ms: u64,
    pub total_ms: u64,
}
impl Totals {
    fn add(&mut self, ms: u64, personal: bool, codex: bool) {
        if personal {
            self.personal_ms = self.personal_ms.saturating_add(ms);
        }
        if codex {
            self.codex_ms = self.codex_ms.saturating_add(ms);
        }
        if personal || codex {
            self.total_ms = self.total_ms.saturating_add(ms);
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Project {
    pub enabled: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub totals: Totals,
    #[serde(default)]
    pub days: BTreeMap<String, Totals>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Saved {
    pub version: u32,
    pub projects: BTreeMap<String, Project>,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            projects: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    Idle,
    Working,
    Waiting,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Event {
    pub session: String,
    pub turn: String,
    pub status: Status,
    pub at_ms: u64,
    #[serde(default)]
    pub starts_turn: bool,
}

struct Agent {
    project: String,
    turn: String,
    status: Status,
    at_ms: u64,
}

pub(super) struct Clock {
    pub saved: Saved,
    pub personal: Option<String>,
    agents: HashMap<(String, String), Agent>,
    pub connected: std::collections::HashSet<String>,
}
impl Clock {
    pub fn new(saved: Saved) -> Self {
        Self {
            saved,
            personal: None,
            agents: HashMap::new(),
            connected: Default::default(),
        }
    }
    pub fn enabled(&self, project: &str) -> bool {
        self.saved.projects.get(project).is_some_and(|p| p.enabled)
    }
    pub fn enable(&mut self, project: &str, enabled: bool) {
        let p = self.saved.projects.entry(project.into()).or_default();
        p.enabled = enabled;
        if !enabled && self.personal.as_deref() == Some(project) {
            self.personal = None;
        }
    }
    pub fn activity(&mut self, project: &str) {
        // A deliberate pause survives pointer movement and automatic activity.
        // Interaction with an untracked project ends the previous project timer.
        if self.personal.as_deref() != Some(project) {
            self.personal = None;
        }
        if self
            .saved
            .projects
            .get(project)
            .is_some_and(|p| p.enabled && !p.paused)
        {
            self.personal = Some(project.into());
        }
    }
    pub fn pause(&mut self, project: &str, pause: bool) {
        if let Some(p) = self.saved.projects.get_mut(project) {
            p.paused = pause;
        }
        if pause {
            if self.personal.as_deref() == Some(project) {
                self.personal = None;
            }
        } else {
            self.activity(project);
        }
    }
    pub fn suspend_personal(&mut self) {
        if let Some(project) = self.personal.clone() {
            self.pause(&project, true);
        }
    }
    pub fn event(&mut self, channel: &str, project: &str, event: Event) {
        self.connected.insert(project.into());
        let id = (channel.to_owned(), event.session.clone());
        if self
            .agents
            .get(&id)
            .is_some_and(|old| old.at_ms > event.at_ms)
        {
            return;
        }
        if self.agents.get(&id).is_some_and(|old| {
            !old.turn.is_empty()
                && !event.turn.is_empty()
                && old.turn != event.turn
                && !event.starts_turn
        }) {
            return;
        }
        self.agents.insert(
            id,
            Agent {
                project: project.into(),
                turn: event.turn,
                status: event.status,
                at_ms: event.at_ms,
            },
        );
    }
    pub fn retain_channels(&mut self, channels: &std::collections::HashSet<String>) {
        self.agents
            .retain(|(channel, _), _| channels.contains(channel));
    }
    pub fn retire(&mut self, project: &str, archived: &str) {
        if self.personal.as_deref() == Some(project) {
            self.personal = None;
        }
        self.agents.retain(|_, a| a.project != project);
        self.connected.remove(project);
        if let Some(mut data) = self.saved.projects.remove(project) {
            data.enabled = false;
            data.paused = true;
            self.saved.projects.insert(archived.into(), data);
        }
    }
    pub fn status(&self, project: &str) -> Status {
        if self
            .agents
            .values()
            .any(|a| a.project == project && a.status == Status::Working)
        {
            Status::Working
        } else if self
            .agents
            .values()
            .any(|a| a.project == project && a.status == Status::Waiting)
        {
            Status::Waiting
        } else {
            Status::Idle
        }
    }
    pub fn advance(&mut self, ms: u64, day: &str) {
        // The controller never backfills time across app downtime, sleep or stalls.
        for (project, data) in &mut self.saved.projects {
            if !data.enabled {
                continue;
            }
            let personal = self.personal.as_ref() == Some(project);
            let codex = self
                .agents
                .values()
                .any(|a| a.project == *project && a.status == Status::Working);
            data.totals.add(ms, personal, codex);
            if personal || codex {
                data.days
                    .entry(day.into())
                    .or_default()
                    .add(ms, personal, codex);
            }
        }
    }
}

pub(super) fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(turn: &str, status: Status, at_ms: u64) -> Event {
        Event {
            session: "session".into(),
            turn: turn.into(),
            status,
            at_ms,
            starts_turn: status == Status::Working,
        }
    }
    #[test]
    fn overlapping_agents_windows_and_personal_time_count_once_per_project() {
        let mut c = Clock::new(Saved::default());
        c.enable("1", true);
        c.enable("2", true);
        c.event("terminal-a", "1", event("a", Status::Working, 1));
        c.event("terminal-b", "1", event("b", Status::Working, 1));
        c.activity("1");
        c.advance(1000, "2026-11-13");
        c.activity("2");
        c.advance(2000, "2026-11-13");
        assert_eq!(
            c.saved.projects["1"].totals,
            Totals {
                personal_ms: 1000,
                codex_ms: 3000,
                total_ms: 3000
            }
        );
        assert_eq!(c.saved.projects["2"].totals.total_ms, 2000);
        c.pause("2", true);
        c.activity("2");
        assert!(c.personal.is_none());
        c.event("terminal-a", "1", event("a", Status::Waiting, 2));
        assert_eq!(c.status("1"), Status::Working);
        c.event("terminal-b", "1", event("b", Status::Idle, 2));
        c.advance(1000, "2026-11-14");
        assert_eq!(c.saved.projects["1"].totals.total_ms, 3000);
        c.enable("1", false);
        c.enable("1", true);
        assert_eq!(c.saved.projects["1"].totals.total_ms, 3000);
    }
    #[test]
    fn late_stops_and_removed_terminals_do_not_leak_time() {
        let mut c = Clock::new(Saved::default());
        c.enable("1", true);
        c.event("a", "1", event("new", Status::Working, 10));
        c.event("a", "1", event("old", Status::Idle, 11));
        assert_eq!(c.status("1"), Status::Working);
        c.retain_channels(&Default::default());
        assert_eq!(c.status("1"), Status::Idle);
        c.activity("1");
        c.suspend_personal();
        c.activity("1");
        assert!(c.personal.is_none());
    }
    #[test]
    fn saved_pauses_and_archives_survive_project_id_reuse_without_resuming_runtime() {
        let mut c = Clock::new(Saved::default());
        c.enable("7", true);
        c.activity("7");
        c.advance(1000, "2026-11-13");
        c.pause("7", true);
        let saved: Saved = serde_json::from_slice(&serde_json::to_vec(&c.saved).unwrap()).unwrap();
        let mut restored = Clock::new(saved);
        restored.activity("7");
        assert!(restored.personal.is_none());
        assert_eq!(restored.status("7"), Status::Idle);
        restored.retire("7", "archived-7");
        restored.enable("7", true);
        assert_eq!(restored.saved.projects["7"].totals.total_ms, 0);
        assert_eq!(restored.saved.projects["archived-7"].totals.total_ms, 1000);
        assert!(!restored.saved.projects["archived-7"].enabled);
    }
}
