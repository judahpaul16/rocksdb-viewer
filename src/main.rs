use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::EnableBlinking,
    event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Table},
    Frame, Terminal,
};
use rocksdb::{DB, IteratorMode, Options};
use serde_json::Value;
use std::{
    io,
    sync::mpsc,
    thread,
    time::Duration,
};

#[derive(Parser)]
#[command(name = "rocksdb-viewer")]
#[command(about = "A general RocksDB browser with TUI")]
struct Args {
    #[arg(short, long)]
    db_path: String,
}

#[derive(Clone, Debug)]
struct Record {
    record_type: String,
    key: String,
    timestamp: i64,
    data: Value,
    raw_data: Vec<u8>,
}

impl Record {
    fn to_table_row(&self, all_headers: &[String]) -> Vec<String> {
        let mut row = vec![self.key.clone()];
        
        if let Value::Object(map) = &self.data {
            for header in &all_headers[1..] {
                if let Some(value) = map.get(header) {
                    row.push(value_to_string(value));
                } else {
                    row.push("".to_string());
                }
            }
        } else {
            for _ in 1..all_headers.len() {
                row.push("".to_string());
            }
        }
        row
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "".to_string(),
        _ => value.to_string(),
    }
}

#[derive(Clone, Debug)]
enum Focus {
    Input,
    Table,
}

struct App {
    records: std::collections::HashMap<String, Vec<Record>>, // type -> records
    headers: std::collections::HashMap<String, Vec<String>>, // type -> headers
    input: String,
    scroll_y: u16,
    focus: Focus,
    selected_table: Option<String>,
    selected_row: Option<usize>,
    receiver: mpsc::Receiver<std::collections::HashMap<String, Vec<Record>>>,
    show_raw_data: Option<String>,
    last_click: Option<(std::time::Instant, String, usize)>, // For tracking double clicks (time, table, row)
}

impl App {
    fn calculate_column_widths(&self, record_type: &str, max_width: u16) -> Vec<Constraint> {
        let headers = match self.headers.get(record_type) {
            Some(h) => h,
            None => return vec![Constraint::Percentage(100)],
        };
        
        let records = match self.records.get(record_type) {
            Some(r) => r,
            None => return vec![Constraint::Percentage(100)],
        };
        
        let mut column_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
        
        for record in records {
            let row_data = record.to_table_row(headers);
            for (i, cell) in row_data.iter().enumerate() {
                if i < column_widths.len() {
                    column_widths[i] = column_widths[i].max(cell.len().min(50)); // Limit to 50 chars
                }
            }
        }
        
        let total_width: usize = column_widths.iter().sum();
        let available_width = max_width as usize;
        
        if total_width < available_width {
            return column_widths.iter()
                .map(|&width| Constraint::Min(width as u16))
                .collect();
        } 
        
        let mut constraints = Vec::new();
        for (i, &width) in column_widths.iter().enumerate() {
            let ratio = width as f32 / total_width as f32;
            let min_width = if i == 0 { 20 } else { 10 }; // Key column gets more space
            let allocated = ((available_width as f32 * ratio) as u16).max(min_width);
            constraints.push(Constraint::Min(allocated));
        }
        
        constraints
    }
    
    fn new(db_path: &str) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let db_path_clone = db_path.to_string();
        thread::spawn(move || {
            loop {
                let mut opts = Options::default();
                opts.create_if_missing(false);
                if let Ok(db) = DB::open_for_read_only(&opts, &db_path_clone, false) {
                    let mut records = std::collections::HashMap::new();
                    let iter = db.iterator(IteratorMode::Start);
                    for item in iter {
                        let (key_bytes, value_bytes) = item.unwrap();
                        let key = String::from_utf8_lossy(&key_bytes).to_string();
                        let value = value_bytes.to_vec();
                        let record = deserialize_record(&key, &value);
                        records.entry(record.record_type.clone()).or_insert_with(Vec::new).push(record);
                    }
                    for recs in records.values_mut() {
                        recs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    }
                    if tx.send(records).is_err() {
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        });

        let mut app = Self {
            records: std::collections::HashMap::new(),
            headers: std::collections::HashMap::new(),
            input: String::new(),
            scroll_y: 0,
            focus: Focus::Input,
            selected_table: None,
            selected_row: None,
            receiver: rx,
            show_raw_data: None,
            last_click: None,
        };

        if let Ok(initial_records) = app.receiver.recv() {
            app.records = initial_records;
            app.collect_headers();
        }

        Ok(app)
    }

    fn collect_headers(&mut self) {
        self.headers.clear();
        for (record_type, records) in &self.records {
            let mut all_keys = std::collections::HashSet::new();
            for record in records {
                if let Value::Object(map) = &record.data {
                    for key in map.keys() {
                        all_keys.insert(key.clone());
                    }
                }
            }
            let mut headers = vec!["key".to_string()];
            let mut keys: Vec<String> = all_keys.into_iter().collect();
            keys.sort();
            headers.extend(keys);
            self.headers.insert(record_type.clone(), headers);
        }
    }
}

fn deserialize_record(key: &str, value: &[u8]) -> Record {
    let record_type = key.split(':').next().unwrap_or("unknown").to_string();

    let timestamp = if record_type == "summary" {
        key.split(':').nth(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0)
    } else {
        key.split(':').nth(2).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0)
    };

    let data = if let Ok(v) = serde_json::from_slice::<Value>(value) {
        v
    } else {
        Value::Object(serde_json::Map::from_iter(vec![("value".to_string(), Value::String(String::from_utf8_lossy(value).to_string()))]))
    };

    Record { record_type, key: key.to_string(), timestamp, data, raw_data: value.to_vec() }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let app = App::new(&args.db_path)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), EnableBlinking, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, app, &args.db_path);

    execute!(terminal.backend_mut(), Clear(ClearType::All))?;
    execute!(terminal.backend_mut(), crossterm::cursor::MoveTo(0, 0))?;
    
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, mut app: App, db_path: &str) -> Result<App, std::io::Error> {
    loop {
        if let Ok(new_records) = app.receiver.try_recv() {
            app.records = new_records;
            app.collect_headers();
        }

        let size = terminal.size()?;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Min(1)].as_ref())
            .split(size);

        if crossterm::event::poll(Duration::from_millis(50))? {
            let event = event::read()?;

            if let Some(_) = app.show_raw_data {
                if let Event::Key(key) = event {
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        return Ok(app);
                    } else if key.code == KeyCode::Char('q') {
                        app.show_raw_data = None;
                    }
                    continue;
                }
            }

            if let Event::Key(key) = event {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(app);
                }
                if matches!(app.focus, Focus::Input) {
                    match key.code {
                        KeyCode::Tab => {
                            app.focus = Focus::Table;
                            let mut types: Vec<String> = app.records.keys().cloned().collect();
                            types.sort();
                            if let Some(table) = types.first() {
                                app.selected_table = Some(table.clone());
                                app.selected_row = Some(0);
                            }
                        }
                        KeyCode::Enter => {}
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Tab => {
                            if let Some(current_table) = &app.selected_table {
                                let mut types: Vec<String> = app.records.keys().cloned().collect();
                                types.sort();
                                if let Some(current_index) = types.iter().position(|t| t == current_table) {
                                    let next_index = (current_index + 1) % types.len();
                                    if next_index > current_index {
                                        app.selected_table = Some(types[next_index].clone());
                                        app.selected_row = Some(0);
                                    } else {
                                        app.focus = Focus::Input;
                                        app.selected_table = None;
                                        app.selected_row = None;
                                    }
                                }
                            } else {
                                app.focus = Focus::Input;
                                app.selected_table = None;
                                app.selected_row = None;
                            }
                        }
                        KeyCode::BackTab => {
                            if let Some(current_table) = &app.selected_table {
                                let mut types: Vec<String> = app.records.keys().cloned().collect();
                                types.sort();
                                if let Some(current_index) = types.iter().position(|t| t == current_table) {
                                    if current_index > 0 {
                                        let prev_index = current_index - 1;
                                        app.selected_table = Some(types[prev_index].clone());
                                        app.selected_row = Some(0);
                                    } else {
                                        app.focus = Focus::Input;
                                        app.selected_table = None;
                                        app.selected_row = None;
                                    }
                                }
                            } else {
                                app.focus = Focus::Input;
                                app.selected_table = None;
                                app.selected_row = None;
                            }
                        }
                        KeyCode::Char('r') => {
                            if let (Some(table), Some(row)) = (&app.selected_table, app.selected_row) {
                                if let Some(records) = app.records.get(table) {
                                    let mut filtered = records.clone();
                                    if !app.input.is_empty() {
                                        filtered.retain(|r| r.key.contains(&app.input));
                                    }
                                    if row < filtered.len() {
                                        let record = &filtered[row];
                                        let pretty_hex = record.raw_data.iter().map(|byte| format!("{:02x}", byte)).collect::<Vec<String>>().join(" ");
                                        app.show_raw_data = Some(format!("Raw data for {}:\n{}", record.key, pretty_hex));
                                    }
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            if let (Some(table), Some(row)) = (app.selected_table.as_ref(), app.selected_row) {
                                if let Some(records) = app.records.get(table) {
                                    let mut filtered = records.clone();
                                    if !app.input.is_empty() {
                                        filtered.retain(|r| r.key.contains(&app.input));
                                    }
                                    if row < filtered.len() {
                                        let key_to_remove = filtered[row].key.clone();
                                        app.show_raw_data = Some(format!("Attempting to delete key: {}", key_to_remove));
                                        
                                        let mut opts = Options::default();
                                        opts.create_if_missing(false);
                                        match DB::open(&opts, db_path) {
                                            Ok(db) => {
                                                if let Err(e) = db.try_catch_up_with_primary() {
                                                    app.show_raw_data = Some(format!("Error catching up with primary: {}", e));
                                                    thread::sleep(Duration::from_millis(1000));
                                                    continue;
                                                }
                                                match db.delete(key_to_remove.as_bytes()) {
                                                    Ok(_) => {
                                                        if let Some(records) = app.records.get_mut(table) {
                                                            records.retain(|r| r.key != key_to_remove);
                                                            app.show_raw_data = Some(format!("Successfully deleted key: {}", key_to_remove));
                                                            
                                                            if records.is_empty() {
                                                                app.selected_table = None;
                                                                app.selected_row = None;
                                                            } else {
                                                                let max_row = records.len().saturating_sub(1);
                                                                app.selected_row = Some(row.min(max_row));
                                                            }
                                                        }
                                                    },
                                                    Err(e) => {
                                                        app.show_raw_data = Some(format!("Error deleting key {}: {}", key_to_remove, e));
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                app.show_raw_data = Some(format!("Error opening DB: {}", e));
                                            }
                                        }
                                        thread::sleep(Duration::from_millis(1000)); // Give time to see the message
                                    }
                                }
                            }
                        },
                        KeyCode::Up => {
                            if let Some(table) = &app.selected_table {
                                if let Some(records) = app.records.get(table) {
                                    let mut filtered = records.clone();
                                    if !app.input.is_empty() {
                                        filtered.retain(|r| r.key.contains(&app.input));
                                    }
                                    if !filtered.is_empty() {
                                        if let Some(row) = app.selected_row {
                                            if row > 0 {
                                                app.selected_row = Some(row - 1);
                                            }
                                        } else {
                                            app.selected_row = Some(0);
                                        }
                                    }
                                }
                            } else {
                                let mut types: Vec<String> = app.records.keys().cloned().collect();
                                types.sort();
                                if let Some(table) = types.first() {
                                    app.selected_table = Some(table.clone());
                                    app.selected_row = Some(0);
                                }
                            }
                        }
                        KeyCode::Down => {
                            if let Some(table) = &app.selected_table {
                                if let Some(records) = app.records.get(table) {
                                    let mut filtered = records.clone();
                                    if !app.input.is_empty() {
                                        filtered.retain(|r| r.key.contains(&app.input));
                                    }
                                    if !filtered.is_empty() {
                                        let max_row = filtered.len().saturating_sub(1);
                                        if let Some(row) = app.selected_row {
                                            if row < max_row {
                                                app.selected_row = Some(row + 1);
                                            }
                                        } else {
                                            app.selected_row = Some(0);
                                        }
                                    }
                                }
                            } else {
                                let mut types: Vec<String> = app.records.keys().cloned().collect();
                                types.sort();
                                if let Some(table) = types.first() {
                                    app.selected_table = Some(table.clone());
                                    app.selected_row = Some(0);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Event::Mouse(mouse_event) = event {
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    if mouse_event.column >= chunks[1].left() && mouse_event.column < chunks[1].right() && mouse_event.row >= chunks[1].top() && mouse_event.row < chunks[1].bottom() {
                        app.focus = Focus::Input;
                        app.selected_table = None;
                        app.selected_row = None;
                    } else {
                        let mut y_offset = chunks[2].y;
                        let mut types: Vec<String> = app.records.keys().cloned().collect();
                        types.sort();
                        for record_type in types {
                            let mut records = app.records.get(&record_type).unwrap().clone();
                            if !app.input.is_empty() {
                                records.retain(|r| r.key.contains(&app.input));
                            }
                            if records.is_empty() {
                                continue;
                            }
                            
                            let max_table_height = (size.height - y_offset).saturating_sub(2);
                            let table_height = 10.min(max_table_height);
                            if table_height == 0 {
                                break;
                            }
                            
                            let table_area = Rect::new(chunks[2].x, y_offset, chunks[2].width, table_height + 2);
                            if mouse_event.column >= table_area.left() && mouse_event.column < table_area.right() && mouse_event.row >= table_area.top() && mouse_event.row < table_area.bottom() {
                                let relative_y = mouse_event.row.saturating_sub(table_area.y + 1);
                                let row_index = if relative_y > 0 { relative_y.saturating_sub(2) as usize } else { 0 };
                                if row_index < records.len() {
                                    // Check for double click
                                    let now = std::time::Instant::now();
                                    if let Some((last_time, last_table, last_row)) = app.last_click {
                                        if now.duration_since(last_time).as_millis() < 500 
                                            && last_table == record_type 
                                            && last_row == row_index {
                                            // Double click detected - show raw data
                                            let record = &records[row_index];
                                            let pretty_hex = record.raw_data.iter()
                                                .map(|byte| format!("{:02x}", byte))
                                                .collect::<Vec<String>>()
                                                .join(" ");
                                            app.show_raw_data = Some(format!("Raw data for {}:\n{}", record.key, pretty_hex));
                                            app.last_click = None;
                                            break;
                                        }
                                    }
                                    // Store click info
                                    app.last_click = Some((now, record_type.clone(), row_index));
                                    app.focus = Focus::Table;
                                    app.selected_table = Some(record_type.clone());
                                    app.selected_row = Some(row_index);
                                    break;
                                }
                            }
                            y_offset += table_height + 3;
                            if y_offset >= chunks[2].bottom() {
                                break;
                            }
                        }
                    }
                }
            }
        }

        terminal.draw(|f| ui(f, &mut app))?;

        if matches!(app.focus, Focus::Input) {
            let cursor_x = chunks[1].x + 1 + app.input.len() as u16;
            let cursor_y = chunks[1].y + 1;
            terminal.set_cursor(cursor_x, cursor_y)?;
            terminal.show_cursor()?;
        } else {
            terminal.hide_cursor()?;
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1)
        ].as_ref())
        .split(size);

    let title_line = Line::from(vec![Span::raw("Filter by Key")]);

    let input = Paragraph::new(app.input.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title_line));
    f.render_widget(input, chunks[1]);

    let title = "Records";
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner_area = block.inner(chunks[2]);
    f.render_widget(block, chunks[2]);

    if let Some(raw_data) = &app.show_raw_data {
        let area = centered_rect(60, 25, size);
        let popup_block = Block::default().title("Raw Data").borders(Borders::ALL);
        let paragraph = Paragraph::new(raw_data.as_str()).block(popup_block);
        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(paragraph, area);

        let status_spans = vec![
            Span::styled("Ctrl+C", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
            Span::raw(": Quit  "),
            Span::styled("q", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
            Span::raw(": Close Raw View")
        ];
        let status_line = Paragraph::new(Line::from(status_spans));
        let status_block = Block::default().style(Style::default().bg(Color::Green));
        f.render_widget(status_line.block(status_block), chunks[3]);
        return;
    }

    let mut y_offset = 0;
    let mut types: Vec<String> = app.records.keys().cloned().collect();
    types.sort();
    for record_type in types {
        let mut records = app.records.get(&record_type).unwrap().clone();
        if !app.input.is_empty() {
            records.retain(|r| r.key.contains(&app.input));
        }
        if records.is_empty() {
            continue;
        }
        let headers = app.headers.get(&record_type).unwrap();
        
        let widths = app.calculate_column_widths(&record_type, inner_area.width.saturating_sub(2));
        
        let rows: Vec<ratatui::widgets::Row> = records.iter().enumerate().map(|(i, r)| {
            let style = if app.selected_table.as_ref() == Some(&record_type) && app.selected_row == Some(i) { Style::default().bg(Color::Blue) } else { Style::default() };
            let cells = r.to_table_row(headers)
                .into_iter()
                .map(|content| {
                    ratatui::widgets::Cell::from(content)
                });
            ratatui::widgets::Row::new(cells).style(style)
        }).collect();
        
        let max_table_height = (size.height - (inner_area.y + y_offset)).saturating_sub(2);
        let table_height = 10.min(max_table_height);
        if table_height == 0 {
            break;
        }
        let start = app.scroll_y as usize;
        let visible_rows: Vec<_> = rows.into_iter().skip(start).take(table_height as usize).collect();
        let table_area = Rect::new(inner_area.x, inner_area.y + y_offset, inner_area.width, table_height + 2);
        let header_cells = headers.iter().cloned().map(ratatui::widgets::Cell::from);
        let header_row = ratatui::widgets::Row::new(header_cells).style(Style::default().fg(Color::Yellow));
        
        let table = Table::new(visible_rows)
            .header(header_row)
            .widths(&widths)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(format!("{} Records", record_type)))
            .column_spacing(3);
        f.render_widget(table, table_area);
        y_offset += table_height + 3;
        if y_offset >= inner_area.height {
            break;
        }
    }

    let mut status_spans = vec![
        Span::styled("Ctrl+C", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
        Span::raw(": Quit  "),
        Span::styled("Tab", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
        Span::raw(": Switch Focus")
    ];
    if app.selected_table.is_some() && app.selected_row.is_some() {
        status_spans.extend_from_slice(&[
            Span::raw("  "),
            Span::styled("r", Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
            Span::raw(": View Raw  "),
            Span::styled("d", Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
            Span::raw(": Delete")
        ]);
    }
    let status_line = Paragraph::new(Line::from(status_spans));
    let status_block = Block::default()
        .style(Style::default().bg(Color::Green));
    f.render_widget(status_line.block(status_block), chunks[3]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

