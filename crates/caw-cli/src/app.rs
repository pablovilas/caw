use caw_core::{Monitor, MonitorEvent, NormalizedSession, PluginRegistry, SessionStatus};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use std::time::Duration;

pub struct App {
    pub sessions: Vec<NormalizedSession>,
    pub selected: usize,
    pub should_quit: bool,
    pub show_history: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
            should_quit: false,
            show_history: false,
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.sessions.is_empty() {
                    self.selected = (self.selected + 1).min(self.sessions.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char('h') => {
                self.show_history = !self.show_history;
            }
            KeyCode::Enter => {
                self.focus_selected();
            }
            _ => {}
        }
    }

    fn focus_selected(&self) {
        if let Some(session) = self.sessions.get(self.selected) {
            if let Some(pid) = session.pid {
                crate::focus::focus_terminal_for_pid(pid);
            }
        }
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

        self.sessions.sort_by_key(|s| match s.status {
            SessionStatus::Working => 0,
            SessionStatus::WaitingInput => 1,
            SessionStatus::Idle => 2,
            SessionStatus::Dead => 3,
        });

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
            // Keyboard/terminal events — immediate response
            Some(Ok(event)) = event_stream.next() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        app.handle_key(key.code);
                        needs_redraw = true;
                    }
                }
            }
            // Monitor events
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
