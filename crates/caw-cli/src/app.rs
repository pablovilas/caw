use caw_core::{Monitor, MonitorEvent, NormalizedSession, PluginRegistry, SessionStatus};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Project,
    App,
    Plugin,
    None,
}

impl GroupBy {
    pub fn label(&self) -> &str {
        match self {
            Self::Project => "project",
            Self::App => "app",
            Self::Plugin => "assistant",
            Self::None => "none",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Project => Self::App,
            Self::App => Self::Plugin,
            Self::Plugin => Self::None,
            Self::None => Self::Project,
        }
    }
}

pub struct App {
    pub sessions: Vec<NormalizedSession>,
    pub selected: usize,
    pub should_quit: bool,
    pub group_by: GroupBy,
}

impl App {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
            should_quit: false,
            group_by: GroupBy::Project,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Ctrl+C
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down => {
                if !self.sessions.is_empty() {
                    self.selected = (self.selected + 1).min(self.sessions.len() - 1);
                }
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                self.focus_selected();
            }
            KeyCode::Char('g') => {
                self.group_by = self.group_by.next();
                self.sort_sessions();
            }
            _ => {}
        }
    }

    fn focus_selected(&self) {
        if let Some(session) = self.sessions.get(self.selected) {
            if let Some(pid) = session.pid {
                caw_core::focus::focus_terminal_for_pid(pid);
            }
        }
    }

    fn status_ord(status: &SessionStatus) -> u8 {
        match status {
            SessionStatus::Working => 0,
            SessionStatus::WaitingInput => 1,
            SessionStatus::Idle => 2,
            SessionStatus::Dead => 3,
        }
    }

    pub fn group_key(&self, s: &NormalizedSession) -> String {
        match self.group_by {
            GroupBy::Project => s.project_path.to_string_lossy().to_string(),
            GroupBy::App => s.app_name.clone().unwrap_or_else(|| "-".to_string()),
            GroupBy::Plugin => s.plugin.clone(),
            GroupBy::None => String::new(),
        }
    }

    pub fn group_header(&self, s: &NormalizedSession) -> String {
        match self.group_by {
            GroupBy::Project => {
                let branch = s
                    .git_branch
                    .as_deref()
                    .map(|b| format!(" @{}", b))
                    .unwrap_or_default();
                format!("{}{}", s.project_name, branch)
            }
            GroupBy::App => s.app_name.clone().unwrap_or_else(|| "-".to_string()),
            GroupBy::Plugin => s.display_name.clone(),
            GroupBy::None => String::new(),
        }
    }

    fn sort_sessions(&mut self) {
        if self.group_by == GroupBy::None {
            self.sessions
                .sort_by_key(|s| Self::status_ord(&s.status));
            return;
        }

        // Compute best status per group
        let mut best_per_group: std::collections::HashMap<String, u8> =
            std::collections::HashMap::new();
        for s in &self.sessions {
            let key = self.group_key(s);
            let ord = Self::status_ord(&s.status);
            best_per_group
                .entry(key)
                .and_modify(|v| *v = (*v).min(ord))
                .or_insert(ord);
        }

        let group_by = self.group_by;
        self.sessions.sort_by(|a, b| {
            let ka = match group_by {
                GroupBy::Project => a.project_path.to_string_lossy().to_string(),
                GroupBy::App => a.app_name.clone().unwrap_or_default(),
                GroupBy::Plugin => a.plugin.clone(),
                GroupBy::None => String::new(),
            };
            let kb = match group_by {
                GroupBy::Project => b.project_path.to_string_lossy().to_string(),
                GroupBy::App => b.app_name.clone().unwrap_or_default(),
                GroupBy::Plugin => b.plugin.clone(),
                GroupBy::None => String::new(),
            };
            let ga = best_per_group.get(&ka).unwrap_or(&3);
            let gb = best_per_group.get(&kb).unwrap_or(&3);
            ga.cmp(gb)
                .then(ka.cmp(&kb))
                .then(Self::status_ord(&a.status).cmp(&Self::status_ord(&b.status)))
        });
    }

    fn apply_event(&mut self, event: MonitorEvent) {
        match event {
            MonitorEvent::Added(session) | MonitorEvent::Updated(session) => {
                if let Some(existing) = self
                    .sessions
                    .iter_mut()
                    .find(|s| s.id == session.id && s.plugin == session.plugin)
                {
                    *existing = session;
                } else {
                    self.sessions.push(session);
                }
            }
            MonitorEvent::Removed { id, plugin } => {
                self.sessions.retain(|s| !(s.id == id && s.plugin == plugin));
            }
            MonitorEvent::Snapshot(sessions) => {
                self.sessions = sessions;
            }
        }

        self.sort_sessions();

        if !self.sessions.is_empty() {
            self.selected = self.selected.min(self.sessions.len() - 1);
        }
    }
}

pub async fn run_tui(registry: PluginRegistry) -> anyhow::Result<()> {
    let monitor = Monitor::new(registry);
    let mut rx = monitor.subscribe();

    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut app = App::new();
    app.sessions = monitor.snapshot().await;
    app.sort_sessions();

    let mut terminal = ratatui::init();
    terminal.clear()?;

    let result = run_event_loop(&mut terminal, &mut app, &mut rx).await;

    ratatui::restore();
    result
}

async fn run_event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    rx: &mut tokio::sync::broadcast::Receiver<MonitorEvent>,
) -> anyhow::Result<()> {
    let mut event_stream = EventStream::new();

    terminal.draw(|frame| crate::ui::draw(frame, app))?;

    loop {
        let mut needs_redraw = false;

        tokio::select! {
            Some(Ok(event)) = event_stream.next() => {
                if let Event::Key(key) = event {
                    app.handle_key_event(key);
                    needs_redraw = true;
                }
            }
            result = rx.recv() => {
                match result {
                    Ok(monitor_event) => {
                        app.apply_event(monitor_event);
                        needs_redraw = true;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        }

        if needs_redraw {
            terminal.draw(|frame| crate::ui::draw(frame, app))?;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
