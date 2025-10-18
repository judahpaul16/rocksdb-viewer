use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::EnableBlinking,
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
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
#[command(name = "db_browser")]
#[command(about = "A general RocksDB browser with TUI")]
struct Args {
    /// Path to the RocksDB database
    #[arg(short, long)]
    db_path: String,
}

#[derive(Clone, Debug)]
struct Record {
    record_type: String,
    key: String,
    timestamp: i64,
    data: Value,
}

impl Record {
    fn to_table_row(&self, all_headers: &[String]) -> Vec<String> {
        let mut row = vec![self.key.clone()];
        if let Value::Object(map) = &self.data {
            for header in &all_headers[1..] { // skip "key"
                if let Some(value) = map.get(header) {
                    row.push(value_to_string(value));
                } else {
                    row.push("".to_string());
                }
            }
        } else {
            // For non-object, just put the string representation
            row.push(value_to_string(&self.data));
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

struct App {
    records: std::collections::HashMap<String, Vec<Record>>, // type -> records
    headers: std::collections::HashMap<String, Vec<String>>, // type -> headers
    input: String,
    scroll_y: u16,
    scroll_x: u16,
    receiver: mpsc::Receiver<std::collections::HashMap<String, Vec<Record>>>,
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
        
        // Start with header lengths
        let mut column_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
        
        // Find the maximum length for each column
        for record in records {
            let row_data = record.to_table_row(headers);
            for (i, cell) in row_data.iter().enumerate() {
                if i < column_widths.len() {
                    column_widths[i] = column_widths[i].max(cell.len().min(50)); // Limit to 50 chars
                }
            }
        }
        
        // Adjust based on available space
        let total_width: usize = column_widths.iter().sum();
        let available_width = max_width as usize;
        
        // If we have more space than needed, just use the calculated widths
        if total_width < available_width {
            return column_widths.iter()
                .map(|&width| Constraint::Min(width as u16))
                .collect();
        } 
        
        // Otherwise, distribute proportionally with minimum widths
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
                    // Sort records by timestamp descending (latest first)
                    for recs in records.values_mut() {
                        recs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    }
                    if tx.send(records).is_err() {
                        break; // Exit if the receiver has been dropped
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
            scroll_x: 0,
            receiver: rx,
        };

        // Wait for initial load
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
            let mut headers = vec!["key".to_string()]; // Add key column
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

    Record { record_type, key: key.to_string(), timestamp, data }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let app = App::new(&args.db_path)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), EnableBlinking, LeaveAlternateScreen)?;
    // Note: EnableMouseCapture removed to allow normal text selection
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        // DisableMouseCapture removed since we're not capturing mouse
    )?;
    terminal.show_cursor()?;

    // Clear the screen and print records
    execute!(std::io::stdout(), Clear(ClearType::All))?;

    if let Ok(app) = res {
        // Print records to stdout for selection after quit
        let mut types: Vec<String> = app.records.keys().cloned().collect();
        types.sort();
        for record_type in types {
            let records = app.records.get(&record_type).unwrap();
            if records.is_empty() {
                continue;
            }
            println!("{} Records", record_type);
            if let Some(headers) = app.headers.get(&record_type) {
                println!("{}", headers.join("\t"));
                for record in records {
                    let row = record.to_table_row(headers);
                    println!("{}", row.join("\t"));
                }
            }
            println!();
        }
    } else if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, mut app: App) -> Result<App, std::io::Error> {
    loop {
        // Check for new records from background thread
        if let Ok(new_records) = app.receiver.try_recv() {
            app.records = new_records;
            app.collect_headers();
        }

        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(app),
                    KeyCode::Enter => {
                        // Filter update
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Up => {
                        app.scroll_y = app.scroll_y.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        app.scroll_y += 1;
                    }
                    KeyCode::Left => {
                        app.scroll_x = app.scroll_x.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        app.scroll_x += 1;
                    }
                    KeyCode::PageUp => {
                        app.scroll_y = app.scroll_y.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.scroll_y += 10;
                    }
                    _ => {}
                }
            }
        }

        terminal.draw(|f| ui(f, &mut app))?;

        // Set cursor for input
        let size = terminal.size()?;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Min(1)].as_ref())
            .split(size);
        let cursor_x = chunks[1].x + 1 + app.input.len() as u16;
        let cursor_y = chunks[1].y + 1;
        terminal.set_cursor(cursor_x, cursor_y)?;
        terminal.show_cursor()?;
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Min(1)].as_ref())
        .split(size);

    let input = Paragraph::new(app.input.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Filter by Key ")
            .title_style(Style::default())
            .title(ratatui::text::Span::styled(
                "(Press 'q' to quit)", 
                Style::default().fg(Color::Red)
            )));
    f.render_widget(input, chunks[1]);

    let title = "Records";
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner_area = block.inner(chunks[2]);
    f.render_widget(block, chunks[2]);

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
        
        // Get dynamic column widths based on content
        let widths = app.calculate_column_widths(&record_type, inner_area.width.saturating_sub(2));
        
        // Create rows with cell wrapping
        let rows: Vec<ratatui::widgets::Row> = records.iter().map(|r| {
            let cells = r.to_table_row(headers)
                .into_iter()
                .map(|content| {
                    // Create cell with wrapping enabled
                    ratatui::widgets::Cell::from(content)
                });
            ratatui::widgets::Row::new(cells)
        }).collect();
        
        let table_height = 10;
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
}

