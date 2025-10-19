use crate::data::{DataManager, FullDataLoader};
use crate::models::Record;
use ratatui::layout::Constraint;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum Focus {
    Input,
    TableSelect,
    Table,
}

pub struct App {
    pub data_manager: DataManager<FullDataLoader>,
    pub input: String,
    pub scroll_y: u16,
    pub focus: Focus,
    pub selected_table: Option<String>,
    pub selected_row: Option<usize>,
    pub show_raw_data: Option<String>,
    pub should_quit: bool,
    pub last_click: Option<(Instant, String, usize)>,
    pub table_select_index: usize,
}

impl App {
    pub fn new(db_path: &str) -> Self {
        let loader = FullDataLoader::new(db_path.to_string());
        let mut data_manager = DataManager::new(loader);
        data_manager.start_background_loading();
        if let Ok(initial_records) = data_manager.rx.recv() {
            data_manager.records = initial_records;
            data_manager.collect_headers();
        }

        Self {
            data_manager,
            input: String::new(),
            scroll_y: 0,
            focus: Focus::TableSelect,
            selected_table: None,
            selected_row: None,
            show_raw_data: None,
            last_click: None,
            table_select_index: 0,
            should_quit: false,
        }
    }

    pub fn calculate_column_widths(&self, record_type: &str, max_width: u16) -> Vec<Constraint> {
        let headers = match self.data_manager.get_headers().get(record_type) {
            Some(h) => h,
            None => return vec![Constraint::Percentage(100)],
        };

        let records = match self.data_manager.get_records().get(record_type) {
            Some(r) => r,
            None => return vec![Constraint::Percentage(100)],
        };

        let mut column_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

        for record in records {
            let row_data = record.to_table_row(headers);
            for (i, cell) in row_data.iter().enumerate() {
                if i < column_widths.len() {
                    column_widths[i] = column_widths[i].max(cell.len().min(50));
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
            let min_width = if i == 0 { 20 } else { 10 };
            let allocated = ((available_width as f32 * ratio) as u16).max(min_width);
            constraints.push(Constraint::Min(allocated));
        }

        constraints
    }

    pub fn get_filtered_records(&self, record_type: &str) -> Vec<Record> {
        let mut records = self.data_manager.get_records().get(record_type).unwrap().clone();
        if !self.input.is_empty() {
            records.retain(|r| r.key.contains(&self.input));
        }
        records
    }
}