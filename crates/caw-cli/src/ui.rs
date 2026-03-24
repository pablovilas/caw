use crate::app::App;
use caw_core::SessionStatus;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::path::PathBuf;

const TEAL: Color = Color::Rgb(29, 158, 117);
const AMBER: Color = Color::Rgb(239, 159, 39);
const GRAY: Color = Color::Rgb(136, 135, 128);
const RED: Color = Color::Rgb(226, 75, 74);

fn status_color(status: &SessionStatus) -> Color {
    match status {
        SessionStatus::Working => TEAL,
        SessionStatus::WaitingInput => AMBER,
        SessionStatus::Idle => GRAY,
        SessionStatus::Dead => RED,
    }
}

fn format_tokens(total: u64) -> String {
    if total == 0 {
        "-".to_string()
    } else if total > 1_000_000 {
        format!("{:.1}M", total as f64 / 1_000_000.0)
    } else if total > 1_000 {
        format!("{:.1}k", total as f64 / 1_000.0)
    } else {
        format!("{}", total)
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_sessions(frame, chunks[1], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let working = app
        .sessions
        .iter()
        .filter(|s| s.status == SessionStatus::Working)
        .count();
    let waiting = app
        .sessions
        .iter()
        .filter(|s| s.status == SessionStatus::WaitingInput)
        .count();
    let idle = app
        .sessions
        .iter()
        .filter(|s| s.status == SessionStatus::Idle)
        .count();

    let header = Line::from(vec![
        Span::styled(
            "  caw ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("{} working", working), Style::default().fg(TEAL)),
        Span::raw("  "),
        Span::styled(format!("{} waiting", waiting), Style::default().fg(AMBER)),
        Span::raw("  "),
        Span::styled(format!("{} idle", idle), Style::default().fg(GRAY)),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(header).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_sessions(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width as usize;

    // Column widths (consistent between header and data)
    const COL_STATUS: usize = 14;
    const COL_PLUGIN: usize = 18;
    const COL_APP: usize = 12;
    const COL_TOKENS: usize = 10;
    let col_fixed = COL_STATUS + COL_PLUGIN + COL_APP + COL_TOKENS;
    let col_msg = width.saturating_sub(col_fixed);

    let mut lines: Vec<Line> = Vec::new();

    // Column header
    let h = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    lines.push(Line::from(vec![
        Span::styled(format!(" {:<w$}", "STATUS", w = COL_STATUS - 1), h),
        Span::styled(format!("{:<w$}", "PLUGIN", w = COL_PLUGIN), h),
        Span::styled(format!("{:<w$}", "APP", w = COL_APP), h),
        Span::styled(format!("{:<w$}", "LAST MESSAGE", w = col_msg), h),
        Span::styled(format!("{:>w$}", "TOKENS", w = COL_TOKENS), h),
    ]));

    let mut current_project: Option<PathBuf> = None;
    let mut session_idx: usize = 0;

    for session in &app.sessions {
        // Group header
        if current_project.as_ref() != Some(&session.project_path) {
            current_project = Some(session.project_path.clone());

            let branch_str = session
                .git_branch
                .as_deref()
                .map(|b| format!(" @{}", b))
                .unwrap_or_default();

            let label = format!(" {}{} ", session.project_name, branch_str);
            let pad_len = width.saturating_sub(label.len() + 2);
            let padding = "─".repeat(pad_len);

            lines.push(Line::from(vec![
                Span::styled("──", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    label,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(padding, Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Session row
        let is_selected = session_idx == app.selected;
        let style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let sc = status_color(&session.status);

        let tokens = format_tokens(session.token_usage.total());
        let app_name = session.app_name.as_deref().unwrap_or("-");

        let last_msg = session
            .last_message
            .as_deref()
            .unwrap_or("")
            .replace('\n', " ");
        let last_msg: String = last_msg.chars().take(col_msg.saturating_sub(1)).collect();

        // Build fixed-width spans matching header columns
        let status_label = format!("{} {}", session.status.symbol(), session.status.label());
        let status_text = format!(" {:<w$}", status_label, w = COL_STATUS - 1);
        let plugin_text = format!("{:<w$}", session.display_name, w = COL_PLUGIN);
        let app_text = format!("{:<w$}", app_name, w = COL_APP);
        let msg_text = format!("{:<w$}", last_msg, w = col_msg);
        let token_text = format!("{:>w$}", tokens, w = COL_TOKENS);

        lines.push(Line::from(vec![
            Span::styled(status_text, style.fg(sc)),
            Span::styled(plugin_text, style),
            Span::styled(app_text, style.fg(Color::DarkGray)),
            Span::styled(msg_text, style.fg(GRAY)),
            Span::styled(token_text, style),
        ]));

        session_idx += 1;
    }

    let block = Block::default()
        .borders(Borders::NONE)
        .title_bottom(" q:quit  ↑/↓:navigate  enter:focus ");

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
