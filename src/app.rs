use crate::data::{DataManager, PaginatedDataLoader};
use crate::models::Record;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum Focus {
    Input,
    TableSelect,
    Table,
    Pages,
}

pub struct App {
    pub data_manager: DataManager<PaginatedDataLoader>,
    pub input: String,
    pub scroll_y: u16,
    pub focus: Focus,
    pub selected_table: Option<String>,
    pub selected_row: Option<usize>,
    pub show_raw_data: Option<String>,
    pub should_quit: bool,
    pub last_click: Option<(Instant, String, usize)>,
    pub table_select_index: usize,
    pub sort_column: Option<usize>,
    pub sort_ascending: bool,
    pub current_page: usize,
    pub page_focus: bool,
}

impl App {
    pub fn new(db_path: &str) -> Self {
    let loader = PaginatedDataLoader::new(db_path.to_string());
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
            sort_column: None,
            sort_ascending: true,
            current_page: 0,
            page_focus: false,
        }
    }

    pub fn visible_page_indices(&self, total_pages: usize) -> Vec<usize> {
        if total_pages == 0 { return vec![]; }
        let last = total_pages.saturating_sub(1);
        let mut set = std::collections::BTreeSet::new();
        set.insert(0);
        set.insert(last);
        let start = self.current_page.saturating_sub(3);
        let end = (self.current_page + 3).min(last);
        for i in start..=end { set.insert(i); }
        if self.current_page < 5 {
            for i in 0..=5.min(last) { set.insert(i); }
        }
        if last > 5 && self.current_page > last.saturating_sub(5) {
            for i in last.saturating_sub(5)..=last { set.insert(i); }
        }
        set.into_iter().collect()
    }
    pub fn calculate_column_widths(&self, record_type: &str, max_width: u16) -> Vec<u16> {
        let headers = match self.data_manager.get_headers().get(record_type) {
            Some(h) => h,
            None => return vec![max_width],
        };

        let records = match self.data_manager.get_records().get(record_type) {
            Some(r) => r,
            None => return vec![max_width],
        };

        let mut column_widths: Vec<usize> = headers.iter().enumerate().map(|(i, h)| {
            let base_len = h.len() + 1;
            if self.sort_column == Some(i) { base_len + 3 } else { base_len }
        }).collect();

        for record in records {
            let row_data = record.to_table_row(headers);
            for (i, cell) in row_data.iter().enumerate() {
                if i < column_widths.len() {
                    let cell_width = if self.sort_column == Some(i) {
                        cell.len() + 2
                    } else {
                        cell.len()
                    };
                    column_widths[i] = column_widths[i].max(cell_width.min(50));
                }
            }
        }

        let total_width: usize = column_widths.iter().sum();
        let available_width = max_width as usize;

        if total_width < available_width {
            return column_widths.iter()
                .map(|&width| width as u16)
                .collect();
        }

        let mut widths = Vec::new();
        for (i, &width) in column_widths.iter().enumerate() {
            let ratio = width as f32 / total_width as f32;
            let min_width = if i == 0 { 24 } else { 14 };
            let allocated = ((available_width as f32 * ratio) as u16).max(min_width);
            widths.push(allocated);
        }

        widths
    }

    pub fn get_total_pages(&self, record_type: &str, height: u16) -> usize {
        let records = self.get_filtered_records(record_type);
        let records_per_page = height as usize;
        if records.is_empty() {
            1
        } else {
            (records.len() + records_per_page - 1) / records_per_page
        }
    }

    pub fn get_filtered_records(&self, record_type: &str) -> Vec<Record> {
        let mut records = self.data_manager.get_records().get(record_type).unwrap().clone();
        if !self.input.is_empty() {
            records.retain(|r| r.key.contains(&self.input));
        }
        if let Some(sort_col) = self.sort_column {
            records.sort_by(|a, b| {
                let headers = self.data_manager.get_headers().get(record_type).unwrap();
                let a_row = a.to_table_row(headers);
                let b_row = b.to_table_row(headers);
                let a_val = a_row.get(sort_col).map(|s| s.as_str()).unwrap_or("");
                let b_val = b_row.get(sort_col).map(|s| s.as_str()).unwrap_or("");
                
                match (a_val.parse::<f64>(), b_val.parse::<f64>()) {
                    (Ok(a_num), Ok(b_num)) => if self.sort_ascending {
                        a_num.partial_cmp(&b_num).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        b_num.partial_cmp(&a_num).unwrap_or(std::cmp::Ordering::Equal)
                    },
                    _ => if self.sort_ascending {
                        a_val.cmp(b_val)
                    } else {
                        b_val.cmp(a_val)
                    }
                }
            });
        }
        records
    }
}