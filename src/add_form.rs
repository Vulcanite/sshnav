use crate::inventory::Host;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthChoice {
    OpenSshDefault,
    PrivateKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddHostDraft {
    pub alias: String,
    pub hostname: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub proxy_jump: Option<String>,
    pub auth: AuthChoice,
    pub private_key: Option<PathBuf>,
    pub auto_reconnect: bool,
}

pub enum EditHostAction {
    Save(AddHostDraft),
    Delete,
}

pub fn open(existing_groups: &[String], host_aliases: &[String]) -> Result<Option<AddHostDraft>> {
    let mut terminal = TerminalSession::enter()?;
    let mut app = AddForm::new(existing_groups.to_vec(), host_aliases.to_vec());
    run_form(&mut terminal, &mut app)
}

pub fn open_edit(
    existing_groups: &[String],
    host_aliases: &[String],
    host: &Host,
) -> Result<Option<EditHostAction>> {
    let mut terminal = TerminalSession::enter()?;
    let mut app = AddForm::from_host(existing_groups.to_vec(), host_aliases.to_vec(), host);
    run_edit_form(&mut terminal, &mut app)
}

pub fn open_duplicate(
    existing_groups: &[String],
    host_aliases: &[String],
    host: &Host,
    alias: &str,
) -> Result<Option<AddHostDraft>> {
    let mut terminal = TerminalSession::enter()?;
    let mut app = AddForm::from_host(existing_groups.to_vec(), host_aliases.to_vec(), host);
    app.mode = FormMode::Duplicate;
    app.alias = alias.to_string();
    app.field = Field::Alias;
    run_form(&mut terminal, &mut app)
}

fn run_form(terminal: &mut TerminalSession, app: &mut AddForm) -> Result<Option<AddHostDraft>> {
    loop {
        terminal.draw(|frame| draw(frame, frame.area(), app))?;

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
            KeyCode::Up => app.previous_field(),
            KeyCode::Down => app.next_field(),
            KeyCode::Left | KeyCode::Right if app.field == Field::Auth => app.cycle_auth(),
            KeyCode::Char(' ') if app.field == Field::Auth => app.cycle_auth(),
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if app.field == Field::AutoReconnect =>
            {
                app.toggle_auto_reconnect()
            }
            KeyCode::Tab => app.autocomplete(),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match app.submit() {
                    Ok(draft) => return Ok(Some(draft)),
                    Err(message) => app.message = Some(message),
                }
            }
            KeyCode::Enter => app.next_field(),
            KeyCode::Backspace => app.backspace(),
            KeyCode::Char(ch) => app.push(ch),
            _ => {}
        }
    }
}

fn run_edit_form(
    terminal: &mut TerminalSession,
    app: &mut AddForm,
) -> Result<Option<EditHostAction>> {
    loop {
        terminal.draw(|frame| draw(frame, frame.area(), app))?;

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
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if app.confirm_delete_pressed() {
                    return Ok(Some(EditHostAction::Delete));
                }
                continue;
            }
            _ => {
                app.disarm_delete();
                match key.code {
                    KeyCode::Up => app.previous_field(),
                    KeyCode::Down => app.next_field(),
                    KeyCode::Left | KeyCode::Right if app.field == Field::Auth => app.cycle_auth(),
                    KeyCode::Char(' ') if app.field == Field::Auth => app.cycle_auth(),
                    KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                        if app.field == Field::AutoReconnect =>
                    {
                        app.toggle_auto_reconnect()
                    }
                    KeyCode::Tab => app.autocomplete(),
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        match app.submit() {
                            Ok(draft) => return Ok(Some(EditHostAction::Save(draft))),
                            Err(message) => app.message = Some(message),
                        }
                    }
                    KeyCode::Enter => app.next_field(),
                    KeyCode::Backspace => app.backspace(),
                    KeyCode::Char(ch) => app.push(ch),
                    _ => {}
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FormMode {
    Add,
    Edit,
    Duplicate,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Field {
    Alias,
    Hostname,
    User,
    Port,
    Group,
    Tags,
    ProxyJump,
    Auth,
    PrivateKey,
    AutoReconnect,
}

impl Field {
    const ALL: [Self; 10] = [
        Self::Alias,
        Self::Hostname,
        Self::User,
        Self::Port,
        Self::Group,
        Self::Tags,
        Self::ProxyJump,
        Self::Auth,
        Self::PrivateKey,
        Self::AutoReconnect,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Alias => "Alias",
            Self::Hostname => "Hostname/IP",
            Self::User => "User",
            Self::Port => "Port",
            Self::Group => "Group",
            Self::Tags => "Tags",
            Self::ProxyJump => "Jump host",
            Self::Auth => "Auth",
            Self::PrivateKey => "Private key path",
            Self::AutoReconnect => "Auto-reconnect",
        }
    }
}

struct AddForm {
    mode: FormMode,
    alias: String,
    hostname: String,
    user: String,
    port: String,
    group: String,
    tags: String,
    proxy_jump: String,
    auth: AuthChoice,
    private_key: String,
    existing_private_key: bool,
    auto_reconnect: bool,
    field: Field,
    groups: Vec<String>,
    host_aliases: Vec<String>,
    message: Option<String>,
    key_completion_index: usize,
    confirm_delete: bool,
}

impl AddForm {
    fn new(groups: Vec<String>, host_aliases: Vec<String>) -> Self {
        Self {
            mode: FormMode::Add,
            alias: String::new(),
            hostname: String::new(),
            user: String::new(),
            port: "22".to_string(),
            group: String::new(),
            tags: String::new(),
            proxy_jump: String::new(),
            auth: AuthChoice::OpenSshDefault,
            private_key: String::new(),
            existing_private_key: false,
            auto_reconnect: false,
            field: Field::Alias,
            groups,
            host_aliases,
            message: None,
            key_completion_index: 0,
            confirm_delete: false,
        }
    }

    fn from_host(groups: Vec<String>, host_aliases: Vec<String>, host: &Host) -> Self {
        let auth = match (host.has_private_key, host.private_key_source_path.as_ref()) {
            (true, _) | (false, Some(_)) => AuthChoice::PrivateKey,
            _ => AuthChoice::OpenSshDefault,
        };
        Self {
            mode: FormMode::Edit,
            alias: host.alias.clone(),
            hostname: host.hostname.clone(),
            user: host.user.clone().unwrap_or_default(),
            port: host.port.unwrap_or(22).to_string(),
            group: host.group.clone().unwrap_or_default(),
            tags: host.tags.join(", "),
            proxy_jump: host.proxy_jump.clone().unwrap_or_default(),
            auth,
            private_key: if host.has_private_key {
                String::new()
            } else {
                host.private_key_source_path.clone().unwrap_or_default()
            },
            existing_private_key: host.has_private_key,
            auto_reconnect: host.auto_reconnect,
            field: Field::Hostname,
            groups,
            host_aliases,
            message: None,
            key_completion_index: 0,
            confirm_delete: false,
        }
    }

    fn next_field(&mut self) {
        let fields = self.visible_fields();
        let idx = fields
            .iter()
            .position(|field| *field == self.field)
            .unwrap_or_default();
        self.field = fields[(idx + 1) % fields.len()];
        self.message = None;
        self.confirm_delete = false;
    }

    fn previous_field(&mut self) {
        let fields = self.visible_fields();
        let idx = fields
            .iter()
            .position(|field| *field == self.field)
            .unwrap_or_default();
        self.field = fields[(idx + fields.len() - 1) % fields.len()];
        self.message = None;
        self.confirm_delete = false;
    }

    fn cycle_auth(&mut self) {
        self.auth = match self.auth {
            AuthChoice::OpenSshDefault => AuthChoice::PrivateKey,
            AuthChoice::PrivateKey => AuthChoice::OpenSshDefault,
        };
        if self.auth != AuthChoice::PrivateKey && self.field == Field::PrivateKey {
            self.field = Field::Auth;
        }
        self.message = None;
        self.confirm_delete = false;
    }

    fn toggle_auto_reconnect(&mut self) {
        self.auto_reconnect = !self.auto_reconnect;
        self.message = None;
        self.confirm_delete = false;
    }

    fn push(&mut self, ch: char) {
        if let Some(text) = self.active_text_mut() {
            text.push(ch);
        }
        self.key_completion_index = 0;
        self.message = None;
        self.confirm_delete = false;
    }

    fn backspace(&mut self) {
        if let Some(text) = self.active_text_mut() {
            text.pop();
        }
        self.key_completion_index = 0;
        self.message = None;
    }

    fn arm_delete(&mut self) {
        self.confirm_delete = true;
        self.message = Some("press Ctrl-D again to delete this host".to_string());
    }

    fn confirm_delete_pressed(&mut self) -> bool {
        if self.confirm_delete {
            true
        } else {
            self.arm_delete();
            false
        }
    }

    fn disarm_delete(&mut self) {
        if self.confirm_delete {
            self.confirm_delete = false;
            self.message = None;
        }
    }

    fn autocomplete(&mut self) {
        match self.field {
            Field::Group => {
                if let Some(group) = matching_groups(&self.groups, &self.group).first() {
                    self.group.clone_from(group);
                }
            }
            Field::ProxyJump => {
                if let Some(alias) = matching_values(&self.host_aliases, &self.proxy_jump).first() {
                    self.proxy_jump.clone_from(alias);
                }
            }
            Field::PrivateKey => {
                let suggestions = key_path_suggestions(&self.private_key);
                if let Some(suggestion) =
                    suggestions.get(self.key_completion_index % suggestions.len().max(1))
                {
                    self.private_key.clone_from(suggestion);
                    self.key_completion_index += 1;
                }
            }
            _ => self.next_field(),
        }
        self.confirm_delete = false;
    }

    fn submit(&self) -> std::result::Result<AddHostDraft, String> {
        if self.alias.trim().is_empty() {
            return Err("alias is required".to_string());
        }
        if self.hostname.trim().is_empty() {
            return Err("hostname is required".to_string());
        }
        if self.user.trim().is_empty() {
            return Err("user is required".to_string());
        }
        let port = if self.port.trim().is_empty() {
            None
        } else {
            Some(
                self.port
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| "port must be a number from 0 to 65535".to_string())?,
            )
        };
        if self.auth == AuthChoice::PrivateKey
            && self.private_key.trim().is_empty()
            && (self.mode == FormMode::Add || !self.existing_private_key)
        {
            return Err("private key path is required for private key auth".to_string());
        }

        Ok(AddHostDraft {
            alias: self.alias.trim().to_string(),
            hostname: self.hostname.trim().to_string(),
            user: Some(self.user.trim().to_string()),
            port,
            group: optional(self.group.as_str()),
            tags: self
                .tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            proxy_jump: optional(self.proxy_jump.as_str()),
            auth: self.auth.clone(),
            private_key: (self.auth == AuthChoice::PrivateKey
                && !self.private_key.trim().is_empty())
            .then(|| PathBuf::from(self.private_key.trim())),
            auto_reconnect: self.auto_reconnect,
        })
    }

    fn active_text_mut(&mut self) -> Option<&mut String> {
        match self.field {
            Field::Alias if self.mode != FormMode::Edit => Some(&mut self.alias),
            Field::Alias => None,
            Field::Hostname => Some(&mut self.hostname),
            Field::User => Some(&mut self.user),
            Field::Port => Some(&mut self.port),
            Field::Group => Some(&mut self.group),
            Field::Tags => Some(&mut self.tags),
            Field::ProxyJump => Some(&mut self.proxy_jump),
            Field::PrivateKey => Some(&mut self.private_key),
            Field::Auth => None,
            Field::AutoReconnect => None,
        }
    }

    fn visible_fields(&self) -> Vec<Field> {
        Field::ALL
            .into_iter()
            .filter(|field| self.mode != FormMode::Edit || *field != Field::Alias)
            .filter(|field| *field != Field::PrivateKey || self.auth == AuthChoice::PrivateKey)
            .collect()
    }

    fn value_for(&self, field: Field) -> String {
        match field {
            Field::Alias => self.alias.clone(),
            Field::Hostname => self.hostname.clone(),
            Field::User => self.user.clone(),
            Field::Port => self.port.clone(),
            Field::Group => self.group.clone(),
            Field::Tags => self.tags.clone(),
            Field::ProxyJump => self.proxy_jump.clone(),
            Field::Auth => auth_label(&self.auth).to_string(),
            Field::PrivateKey => {
                if self.auth == AuthChoice::PrivateKey {
                    if self.private_key.is_empty() && self.existing_private_key {
                        "(stored encrypted key)".to_string()
                    } else {
                        self.private_key.clone()
                    }
                } else {
                    "-".to_string()
                }
            }
            Field::AutoReconnect => {
                if self.auto_reconnect {
                    "on".to_string()
                } else {
                    "off".to_string()
                }
            }
        }
    }
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

fn draw(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AddForm) {
    let area = upper_centered_rect(area, 124, 29);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(area);

    let title_lines = match app.mode {
        FormMode::Add => vec![Line::from("Add host")],
        FormMode::Edit => vec![Line::from(format!("Edit host: {}", app.alias))],
        FormMode::Duplicate => vec![Line::from(format!("Duplicate host: {}", app.alias))],
    };
    let brand = Span::styled(
        "sshnav",
        Style::default()
            .fg(Color::Rgb(231, 112, 75))
            .add_modifier(Modifier::BOLD),
    );
    let title = Paragraph::new(title_lines).block(
        Block::default()
            .title(Line::from(brand))
            .borders(Borders::ALL),
    );
    frame.render_widget(title, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    draw_fields(frame, body[0], app);
    draw_preview(frame, body[1], app);

    let help = contextual_help(app);
    frame.render_widget(
        Paragraph::new(help).block(Block::default().title("Help").borders(Borders::ALL)),
        chunks[2],
    );

    let footer_text = match app.mode {
        FormMode::Add => "Up/Down move  Enter next  Ctrl-S save  Esc cancel",
        FormMode::Edit => "Up/Down move  Enter next  Ctrl-S save  Ctrl-D delete  Esc cancel",
        FormMode::Duplicate => "Up/Down move  Enter next  Ctrl-S duplicate  Esc cancel",
    };
    let footer = Paragraph::new(footer_text).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}

fn draw_fields(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AddForm) {
    let items = app
        .visible_fields()
        .iter()
        .map(|field| {
            let mut line = if *field == Field::Auth {
                auth_line(app)
            } else {
                field_line(app, *field)
            };
            if *field == app.field {
                line = line.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            ListItem::new(line)
        })
        .collect::<Vec<_>>();
    let list = List::new(items).block(Block::default().title("Fields").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn draw_preview(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AddForm) {
    let mut lines = vec![
        Line::from(format!("Alias: {}", display_or_dash(&app.alias))),
        Line::from(format!("Host: {}", display_or_dash(&app.hostname))),
        Line::from(format!("User: {}", display_or_dash(&app.user))),
        Line::from(format!("Port: {}", display_or_dash(&app.port))),
        Line::from(format!("Group: {}", display_or_dash(&app.group))),
        Line::from(format!("Tags: {}", display_or_dash(&app.tags))),
        Line::from(format!("Jump: {}", display_or_dash(&app.proxy_jump))),
        Line::from(format!("Auth: {}", auth_label(&app.auth))),
    ];
    if app.auth == AuthChoice::PrivateKey {
        if app.existing_private_key {
            lines.push(Line::from("Key: stored encrypted copy"));
            if !app.private_key.trim().is_empty() {
                lines.push(Line::from(format!(
                    "Replace from: {}",
                    truncate(&app.private_key, 42)
                )));
            }
        } else {
            lines.push(Line::from(format!(
                "Key file: {}",
                truncate(&display_or_dash(&app.private_key), 42)
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(required_summary(app)));

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Preview").borders(Borders::ALL)),
        area,
    );
}

fn field_line(app: &AddForm, field: Field) -> Line<'static> {
    let required = matches!(field, Field::Hostname | Field::User)
        || (app.mode != FormMode::Edit && field == Field::Alias);
    let label = if required {
        format!("{} *", field_label(app, field))
    } else {
        field_label(app, field).to_string()
    };
    Line::from(vec![
        Span::raw("  "),
        Span::raw(format!("{label:<18}")),
        Span::raw(app.value_for(field)),
    ])
}

fn auth_line(app: &AddForm) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::raw(format!("{:<18}", "Authentication")),
        segment("OpenSSH default", app.auth == AuthChoice::OpenSshDefault),
        Span::raw("  "),
        segment("Private key", app.auth == AuthChoice::PrivateKey),
    ])
}

fn segment(label: &'static str, selected: bool) -> Span<'static> {
    let text = format!("[{label}]");
    if selected {
        Span::styled(text, Style::default().add_modifier(Modifier::BOLD))
    } else {
        Span::raw(text)
    }
}

fn contextual_help(app: &AddForm) -> Vec<Line<'static>> {
    let mut help = match app.field {
        Field::Alias => vec![Line::from(
            "Alias is the short name you will type in sshnav.",
        )],
        Field::Hostname => vec![Line::from(
            "Enter a DNS hostname or IP address. This is validated on save.",
        )],
        Field::User => vec![Line::from("Required. The SSH username for this host.")],
        Field::Port => vec![Line::from("Optional. Defaults to 22 if left blank.")],
        Field::Group => {
            let suggestions = matching_groups(&app.groups, &app.group);
            if suggestions.is_empty() {
                vec![Line::from(
                    "Type a new group, or press Tab when suggestions appear.",
                )]
            } else {
                vec![
                    Line::from(format!("Suggestions: {}", suggestions.join(", "))),
                    Line::from("Tab accepts the first matching group."),
                ]
            }
        }
        Field::Tags => vec![Line::from(
            "Comma-separated tags, for example: prod, db, oracle.",
        )],
        Field::ProxyJump => {
            let suggestions = matching_values(&app.host_aliases, &app.proxy_jump);
            if suggestions.is_empty() {
                vec![Line::from(
                    "Optional. Enter a saved alias or an OpenSSH ProxyJump value.",
                )]
            } else {
                vec![
                    Line::from(format!("Saved hosts: {}", suggestions.join(", "))),
                    Line::from("Tab accepts the first matching alias."),
                ]
            }
        }
        Field::Auth => vec![Line::from(
            "Left/Right/Space cycles between OpenSSH default and private key.",
        )],
        Field::PrivateKey => {
            let suggestions = key_path_suggestions(&app.private_key);
            if suggestions.is_empty() {
                if app.mode != FormMode::Add && app.existing_private_key {
                    vec![Line::from(
                        "Optional. Type a path only when replacing the stored encrypted key.",
                    )]
                } else {
                    vec![Line::from(
                        "Type a private key path. Tab looks under ~/.ssh by default.",
                    )]
                }
            } else {
                vec![
                    Line::from(format!("Key suggestions: {}", suggestions.join(", "))),
                    Line::from("Tab cycles key path suggestions."),
                ]
            }
        }
        Field::AutoReconnect => vec![Line::from(
            "Reconnect this SSH session up to three times after a confirmed transport loss.",
        )],
    };
    if let Some(message) = &app.message {
        help.push(Line::from(Span::styled(
            format!("Error: {message}"),
            Style::default().fg(Color::LightRed),
        )));
    }
    help
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

fn field_label(app: &AddForm, field: Field) -> &'static str {
    match field {
        Field::PrivateKey if app.mode != FormMode::Add && app.existing_private_key => {
            "Replace key from"
        }
        _ => field.label(),
    }
}

fn required_summary(app: &AddForm) -> &'static str {
    match app.mode {
        FormMode::Add => "Required: Alias, Hostname/IP, User",
        FormMode::Edit => "Required: Hostname/IP, User",
        FormMode::Duplicate => "Required: Alias, Hostname/IP, User",
    }
}

fn display_or_dash(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "-".to_string()
    } else {
        value.to_string()
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

fn optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn auth_label(auth: &AuthChoice) -> &'static str {
    match auth {
        AuthChoice::OpenSshDefault => "OpenSSH default",
        AuthChoice::PrivateKey => "private key",
    }
}

fn matching_groups(groups: &[String], input: &str) -> Vec<String> {
    let needle = input.trim().to_ascii_lowercase();
    groups
        .iter()
        .filter(|group| needle.is_empty() || group.to_ascii_lowercase().contains(&needle))
        .take(5)
        .cloned()
        .collect()
}

fn matching_values(values: &[String], input: &str) -> Vec<String> {
    let input = input.trim().to_ascii_lowercase();
    values
        .iter()
        .filter(|value| input.is_empty() || value.to_ascii_lowercase().starts_with(&input))
        .take(5)
        .cloned()
        .collect()
}

fn key_path_suggestions(input: &str) -> Vec<String> {
    let input = input.trim();
    let (dir, prefix, display_prefix) = key_completion_parts(input);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut suggestions = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".pub") || (!prefix.is_empty() && !name.starts_with(&prefix)) {
                return None;
            }
            Some(format!("{display_prefix}{name}"))
        })
        .take(8)
        .collect::<Vec<_>>();
    suggestions.sort();
    suggestions
}

fn key_completion_parts(input: &str) -> (PathBuf, String, String) {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    if input.is_empty() {
        return (
            PathBuf::from(&home).join(".ssh"),
            String::new(),
            "~/.ssh/".to_string(),
        );
    }
    if let Some(rest) = input.strip_prefix("~/") {
        let path = Path::new(rest);
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let prefix = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let display_parent = if parent.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", parent.display())
        };
        return (
            PathBuf::from(&home).join(parent),
            prefix,
            format!("{display_parent}/"),
        );
    }
    let path = Path::new(input);
    if input.contains('/') {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        return (
            parent.to_path_buf(),
            prefix,
            format!("{}/", parent.display()),
        );
    }
    (
        PathBuf::from(&home).join(".ssh"),
        input.to_string(),
        "~/.ssh/".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_submit_values() {
        let mut form = AddForm::new(vec![], vec![]);
        form.alias = "prod".into();
        form.hostname = "10.0.0.10".into();
        form.user = "ubuntu".into();
        form.port = "2222".into();
        form.group = "work".into();
        form.tags = "prod, db".into();

        let draft = form.submit().unwrap();

        assert_eq!(draft.alias, "prod");
        assert_eq!(draft.port, Some(2222));
        assert_eq!(draft.tags, vec!["prod", "db"]);
        assert_eq!(draft.auth, AuthChoice::OpenSshDefault);
    }

    #[test]
    fn requires_key_path_for_private_key_auth() {
        let mut form = AddForm::new(vec![], vec![]);
        form.alias = "prod".into();
        form.hostname = "example.com".into();
        form.auth = AuthChoice::PrivateKey;

        assert!(form.submit().is_err());
    }

    #[test]
    fn edit_can_keep_existing_encrypted_key_without_source_path() {
        let mut host = Host::new("prod".into(), "example.com".into());
        host.has_private_key = true;
        host.user = Some("ubuntu".into());
        let form = AddForm::from_host(vec![], vec![], &host);

        let draft = form.submit().unwrap();

        assert_eq!(draft.auth, AuthChoice::PrivateKey);
        assert_eq!(draft.private_key, None);
    }

    #[test]
    fn duplicate_form_keeps_key_and_edits_jump_alias() {
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        host.has_private_key = true;
        host.proxy_jump = Some("bastion".into());
        let mut form = AddForm::from_host(vec![], vec!["bastion".into()], &host);
        form.mode = FormMode::Duplicate;
        form.alias = "prod-copy".into();
        form.field = Field::ProxyJump;
        form.proxy_jump = "bas".into();
        form.autocomplete();

        let draft = form.submit().unwrap();
        assert_eq!(draft.alias, "prod-copy");
        assert_eq!(draft.proxy_jump.as_deref(), Some("bastion"));
        assert_eq!(draft.private_key, None);
    }

    #[test]
    fn delete_confirmation_disarms_on_other_actions() {
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        let mut form = AddForm::from_host(vec![], vec![], &host);

        assert!(!form.confirm_delete_pressed());

        form.disarm_delete();
        form.push('x');

        assert!(!form.confirm_delete);
    }

    #[test]
    fn delete_confirmation_requires_two_consecutive_delete_presses() {
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        let mut form = AddForm::from_host(vec![], vec![], &host);

        assert!(!form.confirm_delete_pressed());
        form.disarm_delete();
        assert!(!form.confirm_delete_pressed());
        assert!(form.confirm_delete);
        assert!(form.confirm_delete_pressed());
    }
}
