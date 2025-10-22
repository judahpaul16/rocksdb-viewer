use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Record {
    pub record_type: String,
    pub key: String,
    pub data: Value,
    pub raw_data: Vec<u8>,
}

impl Record {
    pub fn to_table_row(&self, all_headers: &[String]) -> Vec<String> {
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

pub fn deserialize_record(key: &str, value: &[u8]) -> Record {
    let parts: Vec<&str> = key.split(':').collect();
    let record_type = parts.first().unwrap_or(&"unknown").to_string();

    let data = if let Ok(v) = serde_json::from_slice::<Value>(value) {
        v
    } else {
        Value::Object(serde_json::Map::from_iter(vec![("value".to_string(), Value::String(String::from_utf8_lossy(value).to_string()))]))
    };

    Record { record_type, key: key.to_string(), data, raw_data: value.to_vec() }
}