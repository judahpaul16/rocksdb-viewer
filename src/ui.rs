use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Table},
    Frame,
};

pub fn ui(f: &mut Frame, app: &mut App) {
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

    let title_line = Line::from(vec![Span::styled("search:", Style::default().fg(Color::Blue))]);

    let input = Paragraph::new(app.input.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title_line));

    if let Some(raw_data) = &app.show_raw_data {
        let area = centered_rect(60, 25, size);
        let popup_block = Block::default().title(Line::from(vec![Span::styled("raw data", Style::default().fg(Color::Magenta))])).borders(Borders::ALL);
        let paragraph = Paragraph::new(raw_data.as_str())
            .wrap(ratatui::widgets::Wrap { trim: true })
            .block(popup_block);
        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(paragraph, area);

        let status_spans = vec![
            Span::styled("Ctrl+C", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
            Span::raw(": quit  "),
            Span::styled("Esc", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
            Span::raw(": go back")
        ];
        let status_line = Paragraph::new(Line::from(status_spans));
        let status_block = Block::default().style(Style::default().bg(Color::Green));
        f.render_widget(status_line.block(status_block), chunks[3]);
        return;
    }

    f.render_widget(input, chunks[1]);

    if app.focus == crate::app::Focus::TableSelect || (app.focus == crate::app::Focus::Input && app.selected_table.is_none()) {
        let mut types: Vec<String> = app.data_manager.get_records().keys().cloned().collect();
        types.sort();

        let filtered_types = if !app.input.is_empty() {
            types.into_iter().filter(|t| t.contains(&app.input)).collect()
        } else {
            types
        };

        let items: Vec<ListItem> = filtered_types.iter().enumerate().map(|(i, t)| {
            let style = if app.focus == crate::app::Focus::TableSelect && i == app.table_select_index { Style::default().bg(Color::Blue) } else { Style::default() };
            ListItem::new(t.as_str()).style(style)
        }).collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(Line::from(vec![Span::styled("available record types:", Style::default().fg(Color::Magenta))])));
        f.render_widget(list, chunks[2]);
    } else {
        let title = Line::from(vec![Span::styled("records:", Style::default().fg(Color::Magenta))]);
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner_area = block.inner(chunks[2]);
        f.render_widget(block, chunks[2]);

        if let Some(ref record_type) = app.selected_table {
            let mut records = app.data_manager.get_records().get(record_type).unwrap().clone();
            if !app.input.is_empty() {
                records.retain(|r| r.key.contains(&app.input));
            }
            if !records.is_empty() {
                let headers = app.data_manager.get_headers().get(record_type).unwrap();

                let widths = app.calculate_column_widths(&record_type, inner_area.width.saturating_sub(2));

                let rows: Vec<ratatui::widgets::Row> = records.iter().enumerate().map(|(i, r)| {
                    let style = if app.selected_row == Some(i) { Style::default().bg(Color::Blue) } else { Style::default() };
                    let cells = r.to_table_row(headers)
                        .into_iter()
                        .map(|content| {
                            ratatui::widgets::Cell::from(content)
                        });
                    ratatui::widgets::Row::new(cells).style(style)
                }).collect();

                let table_height = (size.height - inner_area.y).saturating_sub(4);

                let visible_rows: Vec<ratatui::widgets::Row> = rows.into_iter()
                    .skip(app.scroll_y as usize)
                    .take(table_height as usize)
                    .collect();
                let table_area = Rect::new(inner_area.x, inner_area.y + 1, inner_area.width, table_height);
                let header_cells = headers.iter().cloned().map(ratatui::widgets::Cell::from);
                let header_row = ratatui::widgets::Row::new(header_cells).style(Style::default().fg(Color::Yellow));

                let table = Table::new(visible_rows)
                    .header(header_row)
                    .widths(&widths)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(format!("{} records", record_type)))
                    .column_spacing(3);
                f.render_widget(table, table_area);
            }
        }
    }

    let mut spans = vec![
        Span::styled("Ctrl+C", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
        Span::raw(": quit  ")
    ];

    match app.focus {
        crate::app::Focus::TableSelect => {
            spans.extend(vec![
                Span::styled("Tab", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": focus search  "),
                Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": select  "),
                Span::styled("Up/Down", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": navigate")
            ]);
        },
        crate::app::Focus::Table => {
            spans.extend(vec![
                Span::styled("Esc", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": go back  "),
                Span::styled("Tab", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": focus search  "),
                Span::styled("r", Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": view raw record value  "),
                Span::styled("d", Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(": delete")
            ]);
        },
        crate::app::Focus::Input => {
            if app.selected_table.is_some() {
                spans.extend(vec![
                    Span::styled("Esc", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::raw(": go back  ")
                ]);
            }
            spans.extend(vec![
                Span::styled("Tab", Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                Span::raw(if app.selected_table.is_none() {
                    ": focus table selection"
                } else {
                    ": focus records"
                })
            ]);
        }
    }
    let status_line = Paragraph::new(Line::from(spans));
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