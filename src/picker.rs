use crate::inventory::{Host, Inventory};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::collections::HashMap;
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

pub enum PickerAction {
    Add,
    Connect(String),
    Edit(String),
    Duplicate(String),
}

pub fn select_host(
    inventory: &Inventory,
    initial_query: Option<String>,
    db_path: &Path,
) -> Result<Option<PickerAction>> {
    let mut terminal = TerminalSession::enter()?;
    let mut app = PickerApp::new(inventory, initial_query.unwrap_or_default(), db_path);

    loop {
        terminal.draw(|frame| {
            app.drain_reachability();
            let area = frame.area();
            draw(frame, area, &mut app);
        })?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(None);
            }
            KeyCode::Enter => {
                if let Some(alias) = app.selected_alias() {
                    return Ok(Some(PickerAction::Connect(alias.to_string())));
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(Some(PickerAction::Add));
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(alias) = app.selected_alias() {
                    return Ok(Some(PickerAction::Edit(alias.to_string())));
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(alias) = app.selected_alias() {
                    return Ok(Some(PickerAction::Duplicate(alias.to_string())));
                }
            }
            KeyCode::Up => app.previous(),
            KeyCode::Down => app.next(),
            KeyCode::Backspace => app.backspace(),
            KeyCode::Char(ch) => app.push(ch),
            _ => {}
        }
    }
}

pub fn rank_hosts<'a>(hosts: &'a [Host], query: &str) -> Vec<(&'a Host, i64)> {
    let matcher = SkimMatcherV2::default();
    let query = query.trim();
    let mut ranked = Vec::new();
    for host in hosts {
        if query.is_empty() {
            ranked.push((host, 0));
            continue;
        }
        let haystack = searchable_text(host);
        if let Some(score) = matcher.fuzzy_match(&haystack, query) {
            ranked.push((host, score));
        }
    }
    ranked.sort_by(|(left_host, left_score), (right_host, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_host.alias.cmp(&right_host.alias))
    });
    ranked
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        self.terminal.draw(f)?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct PickerApp<'a> {
    hosts: &'a [Host],
    query: String,
    ranked: Vec<(&'a Host, i64)>,
    state: ListState,
    db_path: String,
    reachability: HashMap<String, String>,
    reachability_rx: Receiver<(String, String)>,
}

impl<'a> PickerApp<'a> {
    fn new(inventory: &'a Inventory, query: String, db_path: &Path) -> Self {
        let (reachability_tx, reachability_rx) = mpsc::channel();
        let mut reachability = HashMap::new();
        for host in &inventory.hosts {
            let alias = host.alias.clone();
            let hostname = host.hostname.clone();
            let port = host.port.unwrap_or(22);
            reachability.insert(alias.clone(), "checking".to_string());
            let tx = reachability_tx.clone();
            thread::spawn(move || {
                let status = reachability_summary(&hostname, port);
                let _ = tx.send((alias, status));
            });
        }

        let mut app = Self {
            hosts: &inventory.hosts,
            query,
            ranked: Vec::new(),
            state: ListState::default(),
            db_path: db_path.display().to_string(),
            reachability,
            reachability_rx,
        };
        app.refresh();
        app
    }

    fn selected_alias(&self) -> Option<&str> {
        self.state
            .selected()
            .and_then(|idx| self.ranked.get(idx))
            .map(|(host, _)| host.alias.as_str())
    }

    fn push(&mut self, ch: char) {
        self.query.push(ch);
        self.refresh();
    }

    fn backspace(&mut self) {
        self.query.pop();
        self.refresh();
    }

    fn next(&mut self) {
        if self.ranked.is_empty() {
            self.state.select(None);
            return;
        }
        let next = match self.state.selected() {
            Some(idx) if idx + 1 < self.ranked.len() => idx + 1,
            _ => 0,
        };
        self.state.select(Some(next));
    }

    fn previous(&mut self) {
        if self.ranked.is_empty() {
            self.state.select(None);
            return;
        }
        let next = match self.state.selected() {
            Some(0) | None => self.ranked.len() - 1,
            Some(idx) => idx - 1,
        };
        self.state.select(Some(next));
    }

    fn refresh(&mut self) {
        self.ranked = rank_hosts(self.hosts, &self.query);
        if self.ranked.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    fn drain_reachability(&mut self) {
        while let Ok((alias, status)) = self.reachability_rx.try_recv() {
            self.reachability.insert(alias, status);
        }
    }

    fn reachability(&self, host: &Host) -> String {
        self.reachability
            .get(&host.alias)
            .cloned()
            .unwrap_or_else(|| "checking".to_string())
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, area: Rect, app: &mut PickerApp<'_>) {
    if app.hosts.is_empty() {
        draw_empty_state(frame, area, app);
        return;
    }

    let body_height = (app.hosts.len() as u16 + 8).clamp(12, 24);
    let area = upper_centered_rect(area, 118, body_height + 10);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(body_height),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(compact_logo());
    frame.render_widget(header, chunks[0]);

    let input = Paragraph::new(if app.query.is_empty() {
        Line::from(Span::styled(
            "filter hosts...",
            Style::default().add_modifier(Modifier::DIM),
        ))
    } else {
        Line::from(app.query.clone())
    })
    .block(Block::default().title("Filter").borders(Borders::ALL));
    frame.render_widget(input, chunks[1]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[2]);

    let host_block = Block::default().title("Hosts").borders(Borders::ALL);
    let host_inner = host_block.inner(body[0]);
    frame.render_widget(host_block, body[0]);
    let host_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(host_inner);
    frame.render_widget(
        Paragraph::new(host_header()).style(Style::default().add_modifier(Modifier::BOLD)),
        host_chunks[0],
    );

    let items: Vec<ListItem> = app
        .ranked
        .iter()
        .map(|(host, _)| ListItem::new(host_row(host)))
        .collect();

    let list = List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, host_chunks[1], &mut app.state);

    let selected = app
        .state
        .selected()
        .and_then(|idx| app.ranked.get(idx))
        .map(|(host, _)| *host);
    let details = match selected {
        Some(host) => {
            let reachability = app.reachability(host);
            selected_details(host, &reachability)
        }
        None => vec![Line::from("No matching hosts")],
    };
    let detail_title = selected
        .map(|host| format!("Details: {}", truncate(&host.alias, 24)))
        .unwrap_or_else(|| "Details".to_string());
    frame.render_widget(
        Paragraph::new(details).block(Block::default().title(detail_title).borders(Borders::ALL)),
        body[1],
    );

    let footer = Paragraph::new(format!(
        "{} host{}  Enter connect  Ctrl-A add  Ctrl-E edit  Ctrl-D duplicate  arrows move  Esc quit",
        app.hosts.len(),
        if app.hosts.len() == 1 { "" } else { "s" }
    ))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}

fn host_header() -> Line<'static> {
    Line::from(vec![
        Span::raw(format!("{:<20}", "Alias")),
        Span::raw("  "),
        Span::raw(format!("{:<14}", "Group")),
        Span::raw("  "),
        Span::raw(format!("{:<10}", "User")),
        Span::raw("  "),
        Span::raw("Tags"),
    ])
}

fn host_row(host: &Host) -> Line<'static> {
    let mut parts = vec![Span::raw(format!("{:<20}", truncate(&host.alias, 20)))];
    parts.push(Span::raw("  "));
    parts.push(Span::raw(format!(
        "{:<14}",
        truncate(host.group.as_deref().unwrap_or("-"), 14)
    )));
    parts.push(Span::raw("  "));
    parts.push(Span::raw(format!(
        "{:<10}",
        truncate(host.user.as_deref().unwrap_or("-"), 10)
    )));
    parts.push(Span::raw("  "));
    let tags = if host.tags.is_empty() {
        "-".to_string()
    } else {
        host.tags.join(",")
    };
    parts.push(Span::raw(truncate(&tags, 28)));
    Line::from(parts)
}

fn draw_empty_state(frame: &mut ratatui::Frame<'_>, area: Rect, app: &PickerApp<'_>) {
    let mut art = sshnav_logo();
    art.extend([
        Line::from(format!("version {}", env!("CARGO_PKG_VERSION"))),
        Line::from("Designed by sshnav contributors"),
        Line::from(format!("DB: {}", app.db_path)),
        Line::from(""),
        Line::from("Quick start"),
        Line::from("  sshnav host add --alias prod --hostname 10.0.0.10 --user ubuntu"),
        Line::from("  sshnav import ssh-config"),
        Line::from("  sshnav migrate"),
        Line::from("  sshnav doctor"),
        Line::from(""),
        Line::from("Ctrl-A add  Esc/Ctrl-C quit"),
    ]);
    frame.render_widget(
        Paragraph::new(art)
            .alignment(Alignment::Center)
            .block(Block::default().title("sshnav").borders(Borders::ALL)),
        area,
    );
}

fn sshnav_logo() -> Vec<Line<'static>> {
    let logo = [
        "███████ ██████  ██   ██ ███    ██  █████  ██    ██",
        "██      ██      ██   ██ ████   ██ ██   ██ ██    ██",
        "███████ ██████  ███████ ██ ██  ██ ███████ ██    ██",
        "     ██      ██ ██   ██ ██  ██ ██ ██   ██  ██  ██ ",
        "███████ ██████  ██   ██ ██   ████ ██   ██   ████  ",
    ];
    let style = Style::default()
        .fg(Color::Rgb(231, 112, 75))
        .add_modifier(Modifier::BOLD);
    logo.into_iter()
        .map(|line| Line::from(Span::styled(line, style)))
        .chain([Line::from("")])
        .collect()
}

fn compact_logo() -> Vec<Line<'static>> {
    let brand = Style::default()
        .fg(Color::Rgb(231, 112, 75))
        .add_modifier(Modifier::BOLD);
    let muted = Style::default().fg(Color::DarkGray);
    vec![
        Line::from(Span::styled(
            " ____  ____  _   _  _   _    _    __     __",
            brand,
        )),
        Line::from(Span::styled(
            "/ ___|/ ___|| | | || \\ | |  / \\   \\ \\   / /",
            brand,
        )),
        Line::from(Span::styled(
            "\\___ \\\\___ \\| |_| ||  \\| | / _ \\   \\ \\ / / ",
            brand,
        )),
        Line::from(Span::styled(
            " ___) |___) |  _  || |\\  |/ ___ \\   \\ V /  ",
            brand,
        )),
        Line::from(vec![
            Span::styled("|____/|____/|_| |_||_| \\_/_/   \\_\\   \\_/   ", brand),
            Span::styled("  local SSH navigator", muted),
        ]),
        Line::from(""),
    ]
}

fn selected_details(host: &Host, reachability: &str) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""),
        detail_line("Host", &host.hostname),
        detail_line("User", host.user.as_deref().unwrap_or("missing")),
        detail_line("Port", &host.port.unwrap_or(22).to_string()),
        detail_line("Auth", &auth_summary(host)),
        detail_line("Network", reachability),
    ];
    if let Some(group) = &host.group {
        lines.push(detail_line("Group", group));
    }
    if !host.tags.is_empty() {
        lines.push(detail_line("Tags", &host.tags.join(", ")));
    }
    if let Some(proxy_jump) = &host.proxy_jump {
        lines.push(detail_line("ProxyJump", proxy_jump));
    }
    if let Some(display_name) = &host.display_name {
        lines.push(detail_line("Name", display_name));
    }
    lines
}

fn detail_line(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(format!("{label:<10}")),
        Span::raw(truncate(value, 42)),
    ])
}

fn auth_summary(host: &Host) -> String {
    match (
        host.has_private_key,
        host.private_key_source_path.as_deref(),
    ) {
        (true, _) | (false, Some(_)) => "private key".to_string(),
        (false, None) => "OpenSSH default".to_string(),
    }
}

fn reachability_summary(hostname: &str, port: u16) -> String {
    let Ok(mut addrs) = (hostname, port).to_socket_addrs() else {
        return "unknown".to_string();
    };
    let Some(addr) = addrs.next() else {
        return "unknown".to_string();
    };
    match TcpStream::connect_timeout(&addr, Duration::from_millis(250)) {
        Ok(_) => "reachable".to_string(),
        Err(_) => "unreachable".to_string(),
    }
}

fn searchable_text(host: &Host) -> String {
    [
        Some(host.alias.as_str()),
        host.display_name.as_deref(),
        host.group.as_deref(),
        Some(host.hostname.as_str()),
        host.user.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(host.tags.iter().map(String::as_str))
    .collect::<Vec<_>>()
    .join(" ")
}

fn upper_centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.min(max_width);
    let height = area.height.min(max_height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 4,
        width,
        height,
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let mut out = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_alias_tags_groups_hostnames_and_user() {
        let mut prod = Host::new("prod-db".into(), "10.0.0.10".into());
        prod.group = Some("work/prod".into());
        prod.tags = vec!["database".into()];
        prod.user = Some("ubuntu".into());
        let staging = Host::new("staging".into(), "stage.example.com".into());
        let hosts = vec![prod, staging];

        assert_eq!(rank_hosts(&hosts, "database")[0].0.alias, "prod-db");
        assert_eq!(rank_hosts(&hosts, "stage")[0].0.alias, "staging");
        assert_eq!(rank_hosts(&hosts, "ubuntu")[0].0.alias, "prod-db");
        assert_eq!(rank_hosts(&hosts, "work")[0].0.alias, "prod-db");
    }

    #[test]
    fn summarizes_authentication_method() {
        let mut host = Host::new("prod".into(), "example.com".into());
        assert_eq!(auth_summary(&host), "OpenSSH default");

        host.has_private_key = true;
        host.private_key_source_path = Some("~/.ssh/prod".into());
        assert_eq!(auth_summary(&host), "private key");
    }
}
