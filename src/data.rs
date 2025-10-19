use crate::models::{Record, deserialize_record};
use rocksdb::{DB, IteratorMode, Options};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

pub trait DataLoader {
    fn load_records(&self) -> HashMap<String, Vec<Record>>;
    fn has_changed(&self) -> bool;
}

#[derive(Clone)]
pub struct FullDataLoader {
    db_path: String,
    last_load_time: SystemTime,
}

impl FullDataLoader {
    pub fn new(db_path: String) -> Self {
        Self {
            db_path,
            last_load_time: SystemTime::UNIX_EPOCH,
        }
    }
}

impl DataLoader for FullDataLoader {
    fn load_records(&self) -> HashMap<String, Vec<Record>> {
        let mut opts = Options::default();
        opts.create_if_missing(false);
        let mut records = HashMap::new();
        if let Ok(db) = DB::open_for_read_only(&opts, &self.db_path, false) {
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
        }
        records
    }

    fn has_changed(&self) -> bool {
        if let Ok(metadata) = std::fs::metadata(&self.db_path) {
            if let Ok(modified) = metadata.modified() {
                return modified > self.last_load_time;
            }
        }
        false
    }
}

pub struct DataManager<T: DataLoader> {
    loader: T,
    pub records: HashMap<String, Vec<Record>>,
    pub headers: HashMap<String, Vec<String>>,
    tx: mpsc::Sender<HashMap<String, Vec<Record>>>,
    pub rx: mpsc::Receiver<HashMap<String, Vec<Record>>>,
}

impl<T: DataLoader + Send + 'static + Clone> DataManager<T> {
    pub fn new(loader: T) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            loader,
            records: HashMap::new(),
            headers: HashMap::new(),
            tx,
            rx,
        }
    }

    pub fn start_background_loading(&self) {
        let loader = self.loader.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            loop {
                if loader.has_changed() {
                    let records = loader.load_records();
                    if tx.send(records).is_err() {
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(500));
            }
        });
    }

    pub fn try_recv(&mut self) -> bool {
        if let Ok(new_records) = self.rx.try_recv() {
            self.records = new_records;
            self.collect_headers();
            true
        } else {
            false
        }
    }

    pub fn get_records(&self) -> &HashMap<String, Vec<Record>> {
        &self.records
    }

    pub fn get_headers(&self) -> &HashMap<String, Vec<String>> {
        &self.headers
    }

    pub fn delete_record(&mut self, table: &str, key: &str) {
        if let Some(records) = self.records.get_mut(table) {
            records.retain(|r| r.key != key);
        }
    }

    pub fn collect_headers(&mut self) {
        self.headers.clear();
        for (record_type, records) in &self.records {
            let mut all_keys = std::collections::HashSet::new();
            for record in records {
                if let serde_json::Value::Object(map) = &record.data {
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

#[derive(Clone)]
#[allow(dead_code)]
pub struct PaginatedDataLoader {
    db_path: String,
    page_size: usize,
    current_page: usize,
    total_records: usize,
    records: Vec<Record>,
    last_load_time: SystemTime,
}

#[allow(dead_code)]
impl PaginatedDataLoader {
    pub fn new(db_path: String, page_size: usize) -> Self {
        Self {
            db_path,
            page_size,
            current_page: 0,
            total_records: 0,
            records: vec![],
            last_load_time: SystemTime::UNIX_EPOCH,
        }
    }

    pub fn load_page(&mut self, record_type: &str) -> Vec<Record> {
        let mut opts = Options::default();
        opts.create_if_missing(false);
        let mut records = Vec::new();
        if let Ok(db) = DB::open_for_read_only(&opts, &self.db_path, false) {
            let iter = db.iterator(IteratorMode::Start);
            let start = self.current_page * self.page_size;
            let end = start + self.page_size;
            for (i, item) in iter.enumerate() {
                if i < start { continue; }
                if i >= end { break; }
                let (key_bytes, value_bytes) = item.unwrap();
                let key = String::from_utf8_lossy(&key_bytes).to_string();
                let value = value_bytes.to_vec();
                let record = deserialize_record(&key, &value);
                if record.record_type == record_type {
                    records.push(record);
                }
            }
            records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        }
        records
    }

    pub fn next_page(&mut self) {
        self.current_page += 1;
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }
}

impl DataLoader for PaginatedDataLoader {
    fn load_records(&self) -> HashMap<String, Vec<Record>> {
        let mut opts = Options::default();
        opts.create_if_missing(false);
        let mut records = HashMap::new();
        if let Ok(db) = DB::open_for_read_only(&opts, &self.db_path, false) {
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
        }
        records
    }

    fn has_changed(&self) -> bool {
        if let Ok(metadata) = std::fs::metadata(&self.db_path) {
            if let Ok(modified) = metadata.modified() {
                return modified > self.last_load_time;
            }
        }
        false
    }
}