use crate::app::{App, Focus};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use rocksdb::Options;
use std::thread;
use std::time::Duration;

pub fn handle_event(event: Event, app: &mut App, db_path: &str, chunks: &[ratatui::layout::Rect]) {
    if let Some(_) = app.show_raw_data {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                std::process::exit(0);
            } else if key.code == KeyCode::Esc {
                app.show_raw_data = None;
            }
            return;
        } else if let Event::Mouse(mouse_event) = event {
            if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            }
            return;
        }
    }

    if let Event::Key(key) = event {
        handle_key_event(key, app, db_path);
    } else if let Event::Mouse(mouse_event) = event {
        handle_mouse_event(mouse_event, app, chunks);
    }
}

fn handle_key_event(key: crossterm::event::KeyEvent, app: &mut App, db_path: &str) {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    if key.code == KeyCode::Esc && (app.focus == Focus::Table || app.focus == Focus::Input || app.focus == Focus::Pages) {
        app.focus = Focus::TableSelect;
        app.selected_table = None;
        app.selected_row = None;
        return;
    }

    match app.focus {
        Focus::Input => handle_input_key(key, app),
        Focus::TableSelect => handle_table_select_key(key, app),
        Focus::Table => handle_table_key(key, app, db_path),
        Focus::Pages => handle_pages_key(key, app),
    }
}

fn handle_input_key(key: crossterm::event::KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Tab => {
            if app.selected_table.is_none() {
                let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
                types.sort();
                if !types.is_empty() {
                    app.focus = Focus::TableSelect;
                    app.table_select_index = 0;
                }
            } else {
                app.focus = Focus::Table;
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
}

fn handle_table_select_key(key: crossterm::event::KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Tab => {
            app.focus = Focus::Input;
            app.selected_table = None;
            app.selected_row = None;
        }
        KeyCode::Enter => {
            let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
            types.sort();
            if app.table_select_index < types.len() {
                app.selected_table = Some(types[app.table_select_index].clone());
                app.selected_row = Some(0);
                app.focus = Focus::Table;
                app.sort_column = None;
                app.sort_ascending = true;
                app.current_page = 0;
            }
        }
        KeyCode::Up => {
            if app.table_select_index > 0 {
                app.table_select_index -= 1;
            }
        }
        KeyCode::Down => {
            let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
            types.sort();
            if app.table_select_index < types.len().saturating_sub(1) {
                app.table_select_index += 1;
            }
        }
        _ => {}
    }
}

fn handle_pages_key(key: crossterm::event::KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Tab => {
            app.focus = Focus::Input;
            app.page_focus = false;
        },
        KeyCode::Esc => {
            app.focus = Focus::Table;
            app.page_focus = false;
        },
        KeyCode::Left => {
            if app.current_page > 0 {
                app.current_page -= 1;
            }
        },
        KeyCode::Right => {
            if let Some(ref table) = app.selected_table {
                let height = app.rows_per_page.max(1) as u16;
                let total_pages = app.get_total_pages(table, height);
                if app.current_page + 1 < total_pages {
                    app.current_page += 1;
                    // align scroll and selection to first row of the new page if needed
                    let start_idx = app.current_page * app.rows_per_page.max(1);
                    app.scroll_y = start_idx as u16;
                    if let Some(sel) = app.selected_row {
                        if sel < start_idx { app.selected_row = Some(start_idx); }
                    } else {
                        app.selected_row = Some(start_idx);
                    }
                }
            }
        },
        _ => {},
    }
}

fn handle_table_key(key: crossterm::event::KeyEvent, app: &mut App, db_path: &str) {
    match key.code {
        KeyCode::Tab => {
            app.focus = Focus::Pages;
            app.page_focus = true;
        }
        KeyCode::BackTab => {
            if let Some(current_table) = &app.selected_table {
                let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
                types.sort();
                if let Some(current_index) = types.iter().position(|t| t == current_table) {
                    if current_index > 0 {
                        let prev_index = current_index - 1;
                        app.selected_table = Some(types[prev_index].clone());
                        app.selected_row = Some(0);
                        app.sort_column = None;
                        app.sort_ascending = true;
                    } else {
                        app.focus = Focus::Input;
                    }
                }
            } else {
                app.focus = Focus::Input;
            }
        }
        KeyCode::Char('r') => {
            if let (Some(table), Some(row)) = (&app.selected_table, app.selected_row) {
                let filtered = app.get_filtered_records(table);
                if row < filtered.len() {
                    let record = &filtered[row];
                    let pretty_hex = record.raw_data.iter().map(|byte| format!("{:02x}", byte)).collect::<Vec<String>>().join(" ");
                    app.show_raw_data = Some(format!("raw data for {}:\n{}", record.key, pretty_hex));
                }
            }
        }
        KeyCode::PageDown => {
            if let Some(table) = &app.selected_table {
                let height = app.rows_per_page.max(1) as u16;
                let total_pages = app.get_total_pages(table, height);
                if app.current_page + 1 < total_pages {
                    app.current_page += 1;
                    let start_idx = app.current_page * app.rows_per_page.max(1);
                    app.scroll_y = start_idx as u16;
                    app.selected_row = Some(start_idx);
                }
            }
        },
        KeyCode::PageUp => {
            if app.current_page > 0 {
                app.current_page -= 1;
                let start_idx = app.current_page * app.rows_per_page.max(1);
                app.scroll_y = start_idx as u16;
                // move selection to first row of page if it was beyond
                let sel = app.selected_row.unwrap_or(start_idx);
                app.selected_row = Some(sel.max(start_idx));
            }
        },
        KeyCode::Char('d') => {
            if let (Some(table), Some(row)) = (app.selected_table.as_ref(), app.selected_row) {
                let filtered = app.get_filtered_records(table);
                if row < filtered.len() {
                    let key_to_remove = filtered[row].key.clone();
                    app.show_raw_data = Some(format!("Attempting to delete key: {}", key_to_remove));

                    let mut opts = Options::default();
                    opts.create_if_missing(false);
                    match rocksdb::DB::open(&opts, db_path) {
                        Ok(db) => {
                            match db.delete(key_to_remove.as_bytes()) {
                                Ok(_) => {
                                    app.data_manager.delete_record(table, &key_to_remove);
                                    app.show_raw_data = Some(format!("Successfully deleted key: {}", key_to_remove));

                                    if app.data_manager.get_records().get(table).map_or(true, |r| r.is_empty()) {
                                        app.selected_table = None;
                                        app.selected_row = None;
                                    } else {
                                        let max_row = app.data_manager.get_records().get(table).unwrap().len().saturating_sub(1);
                                        app.selected_row = Some(row.min(max_row));
                                    }
                                }
                                Err(e) => {
                                    app.show_raw_data = Some(format!("Error deleting key {}: {}", key_to_remove, e));
                                }
                            }
                        }
                        Err(e) => {
                            app.show_raw_data = Some(format!("Error opening DB: {}", e));
                        }
                    }
                    thread::sleep(Duration::from_millis(1000));
                }
            }
        }
        KeyCode::Up => handle_navigation_up(app),
        KeyCode::Down => handle_navigation_down(app),
        _ => {}
    }
}

fn handle_navigation_up(app: &mut App) {
    if let Some(table) = &app.selected_table {
        let filtered = app.get_filtered_records(table);
        if !filtered.is_empty() {
            if let Some(row) = app.selected_row {
                if row > 0 {
                    let new_row = row - 1;
                    app.selected_row = Some(new_row);
                    let rpp = app.rows_per_page.max(1);
                    let start_idx = app.current_page * rpp;
                    if new_row < start_idx {
                        app.current_page = app.current_page.saturating_sub(1);
                        let new_start = app.current_page * rpp;
                        app.scroll_y = new_start as u16;
                    }
                }
            } else {
                app.selected_row = Some(0);
            }
        }
    } else {
        let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
        types.sort();
        if let Some(table) = types.first() {
            app.selected_table = Some(table.clone());
            app.selected_row = Some(0);
        }
    }
}

fn handle_navigation_down(app: &mut App) {
    if let Some(table) = &app.selected_table {
        let filtered = app.get_filtered_records(table);
        if !filtered.is_empty() {
            let max_row = filtered.len().saturating_sub(1);
            if let Some(row) = app.selected_row {
                if row < max_row {
                    let new_row = row + 1;
                    app.selected_row = Some(new_row);
                    let rpp = app.rows_per_page.max(1);
                    let start_idx = app.current_page * rpp;
                    if new_row >= start_idx + rpp {
                        app.current_page += 1;
                        let new_start = app.current_page * rpp;
                        app.scroll_y = new_start as u16;
                    }
                }
            } else {
                app.selected_row = Some(0);
            }
        }
    } else {
        let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
        types.sort();
        if let Some(table) = types.first() {
            app.selected_table = Some(table.clone());
            app.selected_row = Some(0);
        }
    }
}

fn handle_mouse_event(mouse_event: crossterm::event::MouseEvent, app: &mut App, chunks: &[ratatui::layout::Rect]) {
    if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
        if chunks.len() > 3 && mouse_event.row >= chunks[3].top() && mouse_event.row < chunks[3].bottom() {
            if let Some(table) = &app.selected_table {
                if let Some(records) = app.data_manager.get_records().get(table) {
                    let records_per_page = app.rows_per_page.max(1);
                    let total_pages = (records.len() + records_per_page - 1) / records_per_page;
                    let prefix = " Pages: ";
                    let mut current_x = chunks[3].x + prefix.chars().count() as u16;
                    let indices = app.visible_page_indices(total_pages);
                    let mut iter = indices.iter().peekable();
                    while let Some(&page_idx) = iter.next() {
                        let page_text = format!(" {} ", page_idx + 1);
                        let width = page_text.len() as u16;
                        if mouse_event.column >= current_x && mouse_event.column < current_x + width {
                            app.current_page = page_idx;
                            app.focus = Focus::Pages;
                            app.page_focus = true;
                            let start_idx = page_idx * records_per_page;
                            if !records.is_empty() {
                                let clamped = start_idx.min(records.len().saturating_sub(1));
                                app.selected_row = Some(clamped);
                                app.scroll_y = start_idx as u16;
                            }
                            return;
                        }
                        current_x += width;
                        if let Some(&next_idx) = iter.peek() {
                            if *next_idx > page_idx + 1 { current_x += 3; } else { current_x += 1; }
                        }
                    }
                }
            }
        } else if chunks.len() > 1 && mouse_event.row < chunks[1].bottom() {
            app.focus = Focus::Input;
    } else if chunks.len() > 2 && mouse_event.row >= chunks[2].top() && mouse_event.row < chunks[2].bottom() {
            if app.selected_table.is_some() {
                app.focus = Focus::Table;
                if let Some(table) = &app.selected_table {
                    if chunks.len() <= 2 { return; }
                    
                    let header_y = chunks[2].y + 1;
                    if mouse_event.row == header_y {
                        let start_x = chunks[2].x + 1;
                        let max_width = chunks[2].width.saturating_sub(2);
                        let widths = app.calculate_column_widths(table, max_width);
                        let mut current_x = start_x;
                        for (i, &width) in widths.iter().enumerate() {
                            if mouse_event.column >= current_x && mouse_event.column < current_x + width + 3 {
                                if app.sort_column == Some(i) {
                                    app.sort_ascending = !app.sort_ascending;
                                } else {
                                    app.sort_column = Some(i);
                                    app.sort_ascending = true;
                                }
                                app.selected_row = Some(0);
                                app.scroll_y = 0;
                                app.current_page = 0;
                                break;
                            }
                            current_x += width + 3;
                        }
                    } else {
                        let rows_per_page = app.rows_per_page.max(1);
                        let inner_top = chunks[2].top() + 1; // inside outer block border
                        if mouse_event.row >= inner_top + 1 && mouse_event.row < inner_top + 1 + rows_per_page as u16 {
                            let relative_y = mouse_event.row.saturating_sub(inner_top + 1);
                            let start_idx = app.current_page * rows_per_page;
                            let row_index = start_idx + relative_y as usize;
                            let filtered = app.get_filtered_records(table);
                            if row_index < filtered.len() {
                                let now = std::time::Instant::now();
                                if let Some((last_time, last_table, last_row)) = &app.last_click {
                                    if now.duration_since(*last_time).as_millis() < 500 && *last_table == *table && *last_row == row_index {
                                        let record = &filtered[row_index];
                                        let pretty_hex = record.raw_data.iter().map(|byte| format!("{:02x}", byte)).collect::<Vec<String>>().join(" ");
                                        app.show_raw_data = Some(format!("{}:\n{}", record.key, pretty_hex));
                                        app.last_click = None;
                                    } else {
                                        app.last_click = Some((now, table.clone(), row_index));
                                        app.selected_row = Some(row_index);
                                    }
                                } else {
                                    app.last_click = Some((now, table.clone(), row_index));
                                    app.selected_row = Some(row_index);
                                }
                            }
                        }
                    }
                }
            } else {
                let relative_row = mouse_event.row.saturating_sub(chunks[2].top() + 1);
                let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
                types.sort();
                if relative_row < types.len() as u16 {
                    app.table_select_index = relative_row as usize;
                    app.selected_table = Some(types[app.table_select_index].clone());
                    app.selected_row = Some(0);
                    app.focus = Focus::Table;
                    app.sort_column = None;
                    app.sort_ascending = true;
                }
            }
    } else if app.focus == Focus::Table {
            if let Some(table) = &app.selected_table {
                if mouse_event.row >= chunks[2].top() && mouse_event.row < chunks[2].bottom() {
                    let rows_per_page = app.rows_per_page.max(1);
                    let inner_top = chunks[2].top() + 1;
                    if mouse_event.row >= inner_top + 1 && mouse_event.row < inner_top + 1 + rows_per_page as u16 {
                        let relative_y = mouse_event.row.saturating_sub(inner_top + 1);
                        let start_idx = app.current_page * rows_per_page;
                        let row_index = start_idx + relative_y as usize;
                        let filtered = app.get_filtered_records(table);
                        if row_index < filtered.len() {
                            let now = std::time::Instant::now();
                            if let Some((last_time, last_table, last_row)) = &app.last_click {
                                if now.duration_since(*last_time).as_millis() < 500 && *last_table == *table && *last_row == row_index {
                                    let record = &filtered[row_index];
                                    let pretty_hex = record.raw_data.iter().map(|byte| format!("{:02x}", byte)).collect::<Vec<String>>().join(" ");
                                    app.show_raw_data = Some(format!("Raw data for {}:\n{}", record.key, pretty_hex));
                                    app.last_click = None;
                                } else {
                                    app.last_click = Some((now, table.clone(), row_index));
                                    app.selected_row = Some(row_index);
                                }
                            } else {
                                app.last_click = Some((now, table.clone(), row_index));
                                app.selected_row = Some(row_index);
                            }
                        }
                    }
                }
            }
        }
    }
}