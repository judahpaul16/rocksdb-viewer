mod app;
mod data;
mod events;
mod models;
mod ui;

use crate::app::{App, Focus};
use clap::Parser;
use crossterm::{
    cursor::EnableBlinking,
    event,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

#[derive(Parser)]
#[command(name = "rocksdb-viewer")]
#[command(about = "A general RocksDB browser with TUI")]
struct Args {
    #[arg(short, long)]
    db_path: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let app = App::new(&args.db_path);

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
        if app.data_manager.try_recv() {}

        let size = terminal.size()?;
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(3),
                ratatui::layout::Constraint::Min(1),
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
            ].as_ref())
            .split(size);

        if crossterm::event::poll(Duration::from_millis(50))? {
            let event = event::read()?;
            events::handle_event(event, &mut app, db_path, &chunks);
        }
        if app.should_quit {
            return Ok(app);
    }        terminal.draw(|f| ui::ui(f, &mut app))?;

        if app.focus == Focus::Input {
            let cursor_x = chunks[1].x + 1 + app.input.len() as u16;
            let cursor_y = chunks[1].y + 1;
            terminal.set_cursor(cursor_x, cursor_y)?;
            terminal.show_cursor()?;
        } else {
            terminal.hide_cursor()?;
        }
    }
}
