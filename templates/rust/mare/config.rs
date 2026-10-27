// Reads mare.json (embedded at compile time, so permissions can't be edited after build).
use std::sync::OnceLock;

use serde_json::Value;

const RAW: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../mare.json"));

pub fn get() -> &'static Value {
    static CONFIG: OnceLock<Value> = OnceLock::new();
    CONFIG.get_or_init(|| serde_json::from_str(RAW).unwrap_or(Value::Null))
}

pub fn list(path: &[&str]) -> Option<Vec<String>> {
    let mut v = get();
    for key in path {
        v = v.get(*key)?;
    }
    Some(
        v.as_array()?
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect(),
    )
}

pub fn list_or(path: &[&str], default: &[&str]) -> Vec<String> {
    list(path).unwrap_or_else(|| default.iter().map(|s| s.to_string()).collect())
}
