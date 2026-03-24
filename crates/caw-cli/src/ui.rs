use crate::app::App;
use caw_core::SessionStatus;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

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

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_table(frame, chunks[1], app);
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
        Span::styled("  caw ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
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

fn draw_table(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("STATUS"),
        Cell::from("PLUGIN"),
        Cell::from("APP"),
        Cell::from("PROJECT"),
        Cell::from("BRANCH"),
        Cell::from("LAST MESSAGE"),
        Cell::from("TOKENS"),
    ])
    .style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let status_style = Style::default().fg(status_color(&session.status));
            let row_style = if i == app.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let tokens = if session.token_usage.total() > 0 {
                let total = session.token_usage.total();
                if total > 1_000_000 {
                    format!("{:.1}M", total as f64 / 1_000_000.0)
                } else if total > 1_000 {
                    format!("{:.1}k", total as f64 / 1_000.0)
                } else {
                    format!("{}", total)
                }
            } else {
                "-".to_string()
            };

            let last_msg = session
                .last_message
                .as_deref()
                .unwrap_or("-")
                .chars()
                .take(50)
                .collect::<String>();

            let branch = session.git_branch.as_deref().unwrap_or("-");
            let app = session.app_name.as_deref().unwrap_or("-");

            Row::new(vec![
                Cell::from(format!(
                    "{} {}",
                    session.status.symbol(),
                    session.status.label()
                ))
                .style(status_style),
                Cell::from(session.display_name.as_str()),
                Cell::from(app),
                Cell::from(session.project_name.as_str()),
                Cell::from(branch),
                Cell::from(last_msg),
                Cell::from(tokens),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(18),
        Constraint::Length(14),
        Constraint::Fill(1),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title_bottom(" q:quit  j/k:navigate  enter:focus  h:history  r:refresh "),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(table, area);
}
