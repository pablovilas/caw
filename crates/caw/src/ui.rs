use crate::app::{App, GroupBy};
use crate::palette::{self, BONE, MIST, ASH, GRAPHITE};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

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
    let chunks = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_sessions(frame, chunks[1], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let working = app
        .sessions
        .iter()
        .filter(|s| s.status == caw_core::SessionStatus::Working)
        .count();
    let waiting = app
        .sessions
        .iter()
        .filter(|s| s.status == caw_core::SessionStatus::WaitingInput)
        .count();
    let idle = app
        .sessions
        .iter()
        .filter(|s| s.status == caw_core::SessionStatus::Idle)
        .count();

    let dim = Style::default().fg(ASH);
    let logo_s = Style::default().fg(BONE);
    let bold = Style::default().fg(BONE).add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(vec![
            Span::styled("  ⣠⣶⣖⣶⡖ ", logo_s),
            Span::styled("caw ", bold),
            Span::styled("coding assistant watcher", dim),
            Span::raw("   "),
            Span::styled(format!("● {} ", working), Style::default().fg(palette::WORKING)),
            Span::styled("working  ", Style::default().fg(palette::WORKING)),
            Span::styled(format!("▲ {} ", waiting), Style::default().fg(palette::WAITING)),
            Span::styled("waiting  ", Style::default().fg(palette::WAITING)),
            Span::styled(format!("◉ {} ", idle), Style::default().fg(palette::IDLE)),
            Span::styled("idle", Style::default().fg(palette::IDLE)),
        ]),
        Line::from(vec![
            Span::styled("  ⠸⢿⣿⣿⡛⠁", logo_s),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(dim);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

// Column definition
struct Col {
    name: &'static str,
    width: usize,
    align_right: bool,
}

fn columns_for(group_by: GroupBy) -> Vec<Col> {
    let mut cols = vec![Col { name: "STATUS", width: 14, align_right: false }];

    if group_by != GroupBy::Plugin {
        cols.push(Col { name: "ASSISTANT", width: 18, align_right: false });
    }
    if group_by != GroupBy::App {
        cols.push(Col { name: "APP", width: 12, align_right: false });
    }
    if group_by != GroupBy::Project && group_by != GroupBy::None {
        cols.push(Col { name: "PROJECT", width: 16, align_right: false });
        cols.push(Col { name: "BRANCH", width: 14, align_right: false });
    }
    if group_by == GroupBy::None {
        cols.push(Col { name: "PROJECT", width: 16, align_right: false });
        cols.push(Col { name: "BRANCH", width: 14, align_right: false });
    }

    // LAST MESSAGE is fill — handled separately
    // TOKENS is always last
    cols.push(Col { name: "TOKENS", width: 10, align_right: true });
    cols
}

fn draw_sessions(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width as usize;
    let cols = columns_for(app.group_by);

    let fixed_width: usize = cols.iter().map(|c| c.width).sum();
    let col_msg = width.saturating_sub(fixed_width);

    let mut lines: Vec<Line> = Vec::new();

    // Column header
    let h = Style::default()
        .fg(ASH)
        .add_modifier(Modifier::BOLD);

    let mut hdr_spans: Vec<Span> = Vec::new();
    for (i, col) in cols.iter().enumerate() {
        if col.name == "TOKENS" {
            // Insert LAST MESSAGE before TOKENS
            hdr_spans.push(Span::styled(format!("{:<w$}", "LAST MESSAGE", w = col_msg), h));
        }
        let text = if i == 0 {
            format!(" {:<w$}", col.name, w = col.width - 1)
        } else if col.align_right {
            format!("{:>w$}", col.name, w = col.width)
        } else {
            format!("{:<w$}", col.name, w = col.width)
        };
        hdr_spans.push(Span::styled(text, h));
    }
    lines.push(Line::from(hdr_spans));

    let mut current_group: Option<String> = None;

    for (session_idx, session) in app.sessions.iter().enumerate() {
        // Group header
        if app.group_by != GroupBy::None {
            let group_key = app.group_key(session);
            if current_group.as_ref() != Some(&group_key) {
                current_group = Some(group_key);

                let header_text = app.group_header(session);
                let label = format!(" {} ", header_text);
                let pad_len = width.saturating_sub(label.len() + 2);
                let padding = "─".repeat(pad_len);

                lines.push(Line::from(vec![
                    Span::styled("──", Style::default().fg(GRAPHITE)),
                    Span::styled(
                        label,
                        Style::default()
                            .fg(BONE)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(padding, Style::default().fg(GRAPHITE)),
                ]));
            }
        }

        // Session row
        let is_selected = session_idx == app.selected;
        let style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let sc = palette::status_color(&session.status);

        let tokens = format_tokens(session.token_usage.total());
        let app_name = session.app_name.as_deref().unwrap_or("-");
        let branch = session.git_branch.as_deref().unwrap_or("-");

        let last_msg = session
            .last_message
            .as_deref()
            .unwrap_or("")
            .replace('\n', " ");
        let last_msg: String = last_msg.chars().take(col_msg.saturating_sub(1)).collect();

        let mut spans: Vec<Span> = Vec::new();

        for (i, col) in cols.iter().enumerate() {
            if col.name == "TOKENS" {
                // Insert LAST MESSAGE before TOKENS
                spans.push(Span::styled(
                    format!("{:<w$}", last_msg, w = col_msg),
                    style.fg(ASH),
                ));
            }

            let (text, col_style) = match col.name {
                "STATUS" => {
                    let label = format!("{} {}", session.status.symbol(), session.status.label());
                    (format!(" {:<w$}", label, w = col.width - 1), style.fg(sc))
                }
                "ASSISTANT" => (
                    format!("{:<w$}", session.display_name, w = col.width),
                    style,
                ),
                "APP" => (
                    format!("{:<w$}", app_name, w = col.width),
                    style.fg(MIST),
                ),
                "PROJECT" => (
                    format!("{:<w$}", session.project_name, w = col.width),
                    style,
                ),
                "BRANCH" => (
                    format!("{:<w$}", branch, w = col.width),
                    style.fg(MIST),
                ),
                "TOKENS" => (
                    format!("{:>w$}", tokens, w = col.width),
                    style,
                ),
                _ => (String::new(), style),
            };

            let _ = i; // used for first-col padding already handled in STATUS
            spans.push(Span::styled(text, col_style));
        }

        lines.push(Line::from(spans));
    }

    let footer = format!(
        " q:quit  ↑/↓:navigate  enter:focus  g:group ({}) ",
        app.group_by.label()
    );

    let block = Block::default()
        .borders(Borders::NONE)
        .title_bottom(footer);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
