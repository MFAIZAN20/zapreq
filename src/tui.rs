use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Terminal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::cli::{parse_cli_from, CliArgs};
use crate::collections::{
    list_requests, list_workspace_requests, list_workspaces, CollectionEntry,
};
use crate::config::config_root_dir;
use crate::config::{apply_profile, load_profile, merge_defaults, CliResolved, Config};
use crate::format::{html, json, xml};
use crate::items::parse_request_items;
use crate::output::theme::no_color;
use crate::request::{RequestEngine, RequestSpec};
use crate::utils::{is_binary, normalize_url};

const LEGACY_KEY: &str = "__legacy__";

#[derive(Clone, Debug)]
struct TuiRequest {
    name: String,
    method: String,
    url: String,
    items: Vec<String>,
    headers: HashMap<String, String>,
}

#[derive(Clone, Debug)]
struct TuiResponseView {
    status: u16,
    reason: String,
    body: String,
    headers: String,
    meta: String,
    raw: String,
}

#[derive(Debug)]
struct TuiApp {
    workspace_order: Vec<String>,
    workspace_labels: HashMap<String, String>,
    request_map: HashMap<String, Vec<TuiRequest>>,
    workspace_index: usize,
    request_index: usize,
    filter: String,
    filter_mode: bool,
    tabs: Vec<&'static str>,
    tab_index: usize,
    scroll: u16,
    env_profiles: Vec<Option<String>>,
    env_index: usize,
    response: Option<TuiResponseView>,
    status: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TuiState {
    #[serde(default)]
    workspace_key: Option<String>,
    #[serde(default)]
    tab_index: usize,
    #[serde(default)]
    filter: String,
    #[serde(default)]
    env_profile: Option<String>,
}

impl TuiApp {
    fn new(
        workspace_order: Vec<String>,
        workspace_labels: HashMap<String, String>,
        request_map: HashMap<String, Vec<TuiRequest>>,
        env_profiles: Vec<Option<String>>,
    ) -> Self {
        Self {
            workspace_order,
            workspace_labels,
            request_map,
            workspace_index: 0,
            request_index: 0,
            filter: String::new(),
            filter_mode: false,
            tabs: vec!["Body", "Headers", "Meta", "Raw"],
            tab_index: 0,
            scroll: 0,
            env_profiles,
            env_index: 0,
            response: None,
            status: "Press Enter to run selected request. `/` filters requests. `q` exits."
                .to_string(),
        }
    }

    fn active_workspace_key(&self) -> Option<&str> {
        self.workspace_order
            .get(self.workspace_index)
            .map(|s| s.as_str())
    }

    fn active_workspace_label(&self) -> String {
        self.active_workspace_key()
            .and_then(|k| self.workspace_labels.get(k))
            .cloned()
            .unwrap_or_else(|| "N/A".to_string())
    }

    fn active_env_profile(&self) -> Option<&str> {
        self.env_profiles
            .get(self.env_index)
            .and_then(|v| v.as_deref())
    }

    fn filtered_requests(&self) -> Vec<TuiRequest> {
        let Some(ws) = self.active_workspace_key() else {
            return Vec::new();
        };
        let requests = self.request_map.get(ws).cloned().unwrap_or_default();
        let f = self.filter.trim().to_ascii_lowercase();
        if f.is_empty() {
            return requests;
        }
        requests
            .into_iter()
            .filter(|r| {
                r.name.to_ascii_lowercase().contains(&f)
                    || r.method.to_ascii_lowercase().contains(&f)
                    || r.url.to_ascii_lowercase().contains(&f)
            })
            .collect()
    }

    fn clamp_request_index(&mut self) {
        let len = self.filtered_requests().len();
        if len == 0 {
            self.request_index = 0;
            return;
        }
        if self.request_index >= len {
            self.request_index = len - 1;
        }
    }

    fn selected_request(&self) -> Option<TuiRequest> {
        let requests = self.filtered_requests();
        requests.get(self.request_index).cloned()
    }

    fn apply_state(&mut self, state: &TuiState) {
        if let Some(key) = state.workspace_key.as_deref() {
            if let Some(idx) = self.workspace_order.iter().position(|w| w == key) {
                self.workspace_index = idx;
            }
        }
        self.tab_index = state.tab_index.min(self.tabs.len().saturating_sub(1));
        self.filter = state.filter.clone();
        if let Some(profile) = state.env_profile.as_deref() {
            if let Some(idx) = self
                .env_profiles
                .iter()
                .position(|v| v.as_deref() == Some(profile))
            {
                self.env_index = idx;
            }
        } else if let Some(idx) = self.env_profiles.iter().position(Option::is_none) {
            self.env_index = idx;
        }
        self.request_index = 0;
        self.clamp_request_index();
    }

    fn to_state(&self) -> TuiState {
        TuiState {
            workspace_key: self.active_workspace_key().map(|s| s.to_string()),
            tab_index: self.tab_index,
            filter: self.filter.clone(),
            env_profile: self.active_env_profile().map(|s| s.to_string()),
        }
    }
}

/// Runs the advanced terminal workspace UI.
pub fn run_advanced_tui(config: &Config) -> Result<()> {
    let (workspace_order, workspace_labels, request_map) = load_sources()?;
    if workspace_order.is_empty() {
        println!(
            "No workspaces or legacy requests found. Use `http requests save ...` or `http save ...` first."
        );
        return Ok(());
    }
    let env_profiles = load_env_profile_choices();
    let mut app = TuiApp::new(workspace_order, workspace_labels, request_map, env_profiles);
    if let Ok(state) = load_tui_state() {
        app.apply_state(&state);
    }

    let mut terminal = setup_terminal()?;
    let loop_result = run_event_loop(&mut terminal, &mut app, config);
    let state_result = save_tui_state(&app.to_state());
    let restore_result = restore_terminal(terminal);

    if let Err(err) = loop_result {
        return Err(err);
    }
    if let Err(err) = state_result {
        return Err(err);
    }
    if let Err(err) = restore_result {
        return Err(err);
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).context("failed to initialize terminal")?;
    Ok(terminal)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut TuiApp,
    config: &Config,
) -> Result<()> {
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if handle_key_event(app, key, config)? {
                    break;
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn load_sources() -> Result<(
    Vec<String>,
    HashMap<String, String>,
    HashMap<String, Vec<TuiRequest>>,
)> {
    let workspace_summaries = list_workspaces().context("failed to list workspaces")?;
    let mut order = Vec::new();
    let mut labels = HashMap::new();
    let mut map: HashMap<String, Vec<TuiRequest>> = HashMap::new();

    for ws in workspace_summaries {
        let key = ws.name.clone();
        let requests = list_workspace_requests(&ws.name)
            .with_context(|| format!("failed to load workspace '{}'", ws.name))?;
        let normalized = requests
            .into_iter()
            .map(|req| TuiRequest {
                name: req.name,
                method: req.method,
                url: req.url,
                items: req.items,
                headers: req.headers,
            })
            .collect::<Vec<_>>();
        order.push(key.clone());
        labels.insert(key.clone(), ws.name.clone());
        map.insert(key, normalized);
    }

    let legacy = list_requests().context("failed to list legacy requests")?;
    if !legacy.is_empty() {
        order.push(LEGACY_KEY.to_string());
        labels.insert(LEGACY_KEY.to_string(), "Legacy Aliases".to_string());
        map.insert(LEGACY_KEY.to_string(), legacy_to_requests(legacy));
    }

    Ok((order, labels, map))
}

fn legacy_to_requests(entries: Vec<CollectionEntry>) -> Vec<TuiRequest> {
    entries
        .into_iter()
        .map(|entry| TuiRequest {
            name: entry.alias,
            method: entry.method,
            url: entry.url,
            items: entry.items,
            headers: entry.headers,
        })
        .collect()
}

fn load_env_profile_choices() -> Vec<Option<String>> {
    let mut choices = vec![None];
    let mut names = BTreeSet::new();
    if let Ok(root) = crate::config::config_root_dir() {
        let dir = root.join("envs");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|v| v.to_str()) == Some("json") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        names.insert(name.to_string());
                    }
                }
            }
        }
    }
    for name in names {
        choices.push(Some(name));
    }
    choices
}

fn draw_ui(frame: &mut ratatui::Frame<'_>, app: &TuiApp) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(frame.area());

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(28),
            Constraint::Percentage(50),
        ])
        .split(outer[0]);

    draw_workspace_pane(frame, panes[0], app);
    draw_request_pane(frame, panes[1], app);
    draw_response_pane(frame, panes[2], app);

    let mode = if app.filter_mode { "FILTER" } else { "NORMAL" };
    let env = app.active_env_profile().unwrap_or("none");
    let help = Paragraph::new(format!(
        "[{mode}] workspace:←/→ request:↑/↓ run:Enter tabs:Tab env:e filter:/ clear:Ctrl+u scroll:PgUp/PgDn quit:q | env={env} | {}",
        app.status
    ))
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(help, outer[1]);
}

fn draw_workspace_pane(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &TuiApp) {
    let items = app
        .workspace_order
        .iter()
        .map(|key| {
            let label = app
                .workspace_labels
                .get(key)
                .cloned()
                .unwrap_or_else(|| key.clone());
            ListItem::new(Line::from(label))
        })
        .collect::<Vec<_>>();

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.workspace_index.min(items.len() - 1)));
    }

    let list = List::new(items)
        .block(Block::default().title("Workspaces").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_request_pane(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &TuiApp) {
    let requests = app.filtered_requests();
    let title = format!(
        "Requests [{}] filter={}",
        app.active_workspace_label(),
        if app.filter.is_empty() {
            "<none>"
        } else {
            app.filter.as_str()
        }
    );

    let items = requests
        .iter()
        .map(|req| {
            ListItem::new(Line::from(format!(
                "{}  {} {}",
                req.name, req.method, req.url
            )))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.request_index.min(items.len() - 1)));
    }
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_response_pane(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &TuiApp) {
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let tab_lines = app.tabs.iter().map(|t| Line::from(*t)).collect::<Vec<_>>();
    let tabs = Tabs::new(tab_lines)
        .select(app.tab_index)
        .block(Block::default().title("Response").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, panes[0]);

    let title = if let Some(r) = &app.response {
        format!("{} {}", r.status, r.reason)
    } else {
        "No response yet".to_string()
    };
    let body = current_tab_text(app);
    let paragraph = Paragraph::new(body)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));
    frame.render_widget(paragraph, panes[1]);
}

fn current_tab_text(app: &TuiApp) -> String {
    let Some(resp) = &app.response else {
        return "Select a request and press Enter to execute.".to_string();
    };
    match app.tab_index {
        0 => resp.body.clone(),
        1 => resp.headers.clone(),
        2 => resp.meta.clone(),
        _ => resp.raw.clone(),
    }
}

fn handle_key_event(
    app: &mut TuiApp,
    key: crossterm::event::KeyEvent,
    config: &Config,
) -> Result<bool> {
    if app.filter_mode {
        return handle_filter_mode(app, key);
    }

    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Up => {
            if app.request_index > 0 {
                app.request_index -= 1;
                app.scroll = 0;
            }
        }
        KeyCode::Down => {
            let max = app.filtered_requests().len().saturating_sub(1);
            if app.request_index < max {
                app.request_index += 1;
                app.scroll = 0;
            }
        }
        KeyCode::Left => {
            if app.workspace_index > 0 {
                app.workspace_index -= 1;
                app.request_index = 0;
                app.scroll = 0;
                app.status = format!("Switched workspace to '{}'", app.active_workspace_label());
            }
        }
        KeyCode::Right => {
            if app.workspace_index + 1 < app.workspace_order.len() {
                app.workspace_index += 1;
                app.request_index = 0;
                app.scroll = 0;
                app.status = format!("Switched workspace to '{}'", app.active_workspace_label());
            }
        }
        KeyCode::Enter | KeyCode::Char('r') => {
            let selected = app.selected_request();
            match selected {
                Some(req) => match execute_request_from_tui(&req, app.active_env_profile(), config)
                {
                    Ok(view) => {
                        app.status = format!(
                            "Request completed: {} {} -> {}",
                            req.method, req.url, view.status
                        );
                        app.response = Some(view);
                        app.scroll = 0;
                    }
                    Err(err) => {
                        app.status = format!("Request failed: {err}");
                    }
                },
                None => {
                    app.status = "No request selected.".to_string();
                }
            }
        }
        KeyCode::Tab => {
            app.tab_index = (app.tab_index + 1) % app.tabs.len();
            app.scroll = 0;
        }
        KeyCode::BackTab => {
            if app.tab_index == 0 {
                app.tab_index = app.tabs.len() - 1;
            } else {
                app.tab_index -= 1;
            }
            app.scroll = 0;
        }
        KeyCode::PageDown => {
            app.scroll = app.scroll.saturating_add(10);
        }
        KeyCode::PageUp => {
            app.scroll = app.scroll.saturating_sub(10);
        }
        KeyCode::Char('/') => {
            app.filter_mode = true;
            app.status = "Filter mode: type text and press Enter (Esc to cancel).".to_string();
        }
        KeyCode::Char('e') => {
            if !app.env_profiles.is_empty() {
                app.env_index = (app.env_index + 1) % app.env_profiles.len();
                let active = app.active_env_profile().unwrap_or("none");
                app.status = format!("Active env profile: {active}");
            }
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.filter.clear();
            app.request_index = 0;
            app.status = "Filter cleared.".to_string();
        }
        _ => {}
    }
    app.clamp_request_index();
    Ok(false)
}

fn handle_filter_mode(app: &mut TuiApp, key: crossterm::event::KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.filter_mode = false;
            app.status = "Filter mode canceled.".to_string();
        }
        KeyCode::Enter => {
            app.filter_mode = false;
            app.request_index = 0;
            app.clamp_request_index();
            app.status = format!(
                "Filter applied. {} request(s) match.",
                app.filtered_requests().len()
            );
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.request_index = 0;
            app.clamp_request_index();
        }
        KeyCode::Char(ch) => {
            app.filter.push(ch);
            app.request_index = 0;
            app.clamp_request_index();
        }
        _ => {}
    }
    Ok(false)
}

fn execute_request_from_tui(
    req: &TuiRequest,
    env_profile: Option<&str>,
    config: &Config,
) -> Result<TuiResponseView> {
    let env_map = HashMap::new();
    let mut resolved = CliResolved {
        url: req.url.clone(),
        request_items: req.items.clone(),
        profile_headers: HashMap::new(),
        variables: env_map,
    };
    if let Some(profile_name) = env_profile {
        let profile = load_profile(profile_name)
            .with_context(|| format!("failed to load env profile: {profile_name}"))?;
        apply_profile(&profile, &mut resolved);
    }

    let resolved_url = substitute_placeholders(&resolved.url, &resolved.variables);
    let mut resolved_items = resolved
        .request_items
        .iter()
        .map(|raw| substitute_item_value(raw, &resolved.variables))
        .collect::<Vec<_>>();
    for (k, v) in &resolved.profile_headers {
        resolved_items.push(format!(
            "{}:{}",
            substitute_placeholders(k, &resolved.variables),
            substitute_placeholders(v, &resolved.variables)
        ));
    }
    for (k, v) in &req.headers {
        resolved_items.push(format!(
            "{}:{}",
            substitute_placeholders(k, &resolved.variables),
            substitute_placeholders(v, &resolved.variables)
        ));
    }

    let unresolved = unresolved_placeholders(
        std::iter::once(resolved_url.as_str())
            .chain(resolved_items.iter().map(|s| s.as_str()))
            .collect::<Vec<_>>()
            .as_slice(),
    );
    if !unresolved.is_empty() {
        return Err(anyhow!("unresolved variables: {}", unresolved.join(", ")));
    }

    let usable_url = normalize_url(&resolved_url, &config.default_scheme)
        .context("failed to build usable URL")?;
    let parsed_items =
        parse_request_items(&resolved_items).context("failed to parse request items")?;
    let spec = RequestSpec {
        method: req.method.clone(),
        url: usable_url.clone(),
        items: parsed_items,
    };

    let mut synthetic = vec!["http".to_string(), req.method.clone(), usable_url.clone()];
    synthetic.extend(resolved_items);
    merge_defaults(config, &mut synthetic);
    let mut args: CliArgs =
        parse_cli_from(synthetic).context("failed to parse synthetic CLI args")?;
    args.command = None;

    let engine = RequestEngine::new();
    let started = Instant::now();
    let (trace, response) = engine
        .send(&args, &spec, None)
        .context("request execution failed")?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let headers = response
        .headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let raw = String::from_utf8_lossy(&response.body).into_owned();
    let body = render_body_for_tui(&response.content_type, &response.body);
    let meta = format!(
        "Method: {}\nURL: {}\nStatus: {} {}\nFinal URL: {}\nTime: {} ms\nSize: {} bytes\nContent-Type: {}",
        trace.method,
        trace.url,
        response.status_code,
        response.reason,
        response.final_url,
        elapsed_ms,
        response.body.len(),
        response
            .content_type
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    );

    Ok(TuiResponseView {
        status: response.status_code,
        reason: response.reason,
        body,
        headers,
        meta,
        raw,
    })
}

fn render_body_for_tui(content_type: &Option<String>, body_bytes: &[u8]) -> String {
    if is_binary(body_bytes) {
        return format!("[binary body, {} bytes]", body_bytes.len());
    }

    let content_type = content_type
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let no_color_theme = no_color();

    if content_type.contains("application/json") {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body_bytes) {
            return json::format_json(&value, &no_color_theme, 2, false);
        }
    }
    if content_type.contains("text/xml") || content_type.contains("application/xml") {
        return xml::format_xml(&String::from_utf8_lossy(body_bytes), &no_color_theme);
    }
    if content_type.contains("text/html") {
        return html::format_html(&String::from_utf8_lossy(body_bytes));
    }

    String::from_utf8_lossy(body_bytes).into_owned()
}

fn substitute_placeholders(input: &str, vars: &HashMap<String, String>) -> String {
    let re = regex::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .expect("regex should compile");
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let key = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or_default();
        vars.get(key)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

fn substitute_item_value(raw: &str, vars: &HashMap<String, String>) -> String {
    let token = raw.trim();
    if let Some((k, v)) = token.split_once(":=@") {
        return format!("{}:=@{}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once(":=") {
        return format!("{}:={}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once("==") {
        return format!("{}=={}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once(':') {
        return format!("{}:{}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once("=@") {
        return format!("{}=@{}", k, substitute_placeholders(v, vars));
    }
    if let Some((k, v)) = token.split_once('=') {
        if token.contains('@') && token.contains(";type=") {
        } else {
            return format!("{}={}", k, substitute_placeholders(v, vars));
        }
    }
    if let Some((k, v)) = token.split_once('@') {
        if let Some((path, ct)) = v.split_once(";type=") {
            return format!(
                "{}@{};type={}",
                k,
                substitute_placeholders(path, vars),
                substitute_placeholders(ct, vars)
            );
        }
        return format!("{}@{}", k, substitute_placeholders(v, vars));
    }
    substitute_placeholders(token, vars)
}

fn unresolved_placeholders(values: &[&str]) -> Vec<String> {
    let re = regex::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}|\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .expect("regex should compile");
    let mut unresolved = BTreeSet::new();
    for value in values {
        for caps in re.captures_iter(value) {
            let name = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or_default();
            if !name.is_empty() {
                unresolved.insert(name.to_string());
            }
        }
    }
    unresolved.into_iter().collect()
}

fn load_tui_state() -> Result<TuiState> {
    let path = tui_state_path()?;
    if !path.exists() {
        return Ok(TuiState::default());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read tui state file: {}", path.display()))?;
    let state: TuiState = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse tui state file: {}", path.display()))?;
    Ok(state)
}

fn save_tui_state(state: &TuiState) -> Result<()> {
    let path = tui_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create tui state dir: {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(state).context("failed to serialize tui state")?;
    std::fs::write(&path, raw)
        .with_context(|| format!("failed to write tui state file: {}", path.display()))?;
    Ok(())
}

fn tui_state_path() -> Result<PathBuf> {
    Ok(config_root_dir()?.join("tui_state.json"))
}
