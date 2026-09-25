use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::util::sanitize_snapshot_name;

pub fn write_versioned(current_path: &Path, archive_dir: &Path, value: &Value) -> Result<bool> {
    if let Some(parent) = current_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    if current_path.exists() {
        let old_text = fs::read_to_string(current_path)
            .with_context(|| format!("read {}", current_path.display()))?;
        let old: Value = serde_json::from_str(&old_text)
            .with_context(|| format!("parse {}", current_path.display()))?;

        if semantic(&old) == semantic(value) {
            println!("unchanged {}", current_path.display());
            return Ok(false);
        }

        fs::create_dir_all(archive_dir)
            .with_context(|| format!("create {}", archive_dir.display()))?;
        let stamp = old
            .get("generated_at")
            .and_then(Value::as_str)
            .map(sanitize_snapshot_name)
            .unwrap_or_else(|| "unknown".to_string());
        let archive_path = archive_dir.join(format!("{stamp}.json"));
        if !archive_path.exists() {
            fs::write(&archive_path, pretty(&old)?)
                .with_context(|| format!("archive {}", archive_path.display()))?;
        }
    }

    fs::write(current_path, pretty(value)?)
        .with_context(|| format!("write {}", current_path.display()))?;
    println!("updated {}", current_path.display());
    Ok(true)
}

pub fn write_stable(path: &Path, value: &Value) -> Result<bool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    if path.exists() {
        let old_text =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let old: Value =
            serde_json::from_str(&old_text).with_context(|| format!("parse {}", path.display()))?;
        if semantic(&old) == semantic(value) {
            println!("unchanged {}", path.display());
            return Ok(false);
        }
    }

    fs::write(path, pretty(value)?).with_context(|| format!("write {}", path.display()))?;
    println!("updated {}", path.display());
    Ok(true)
}

fn pretty(value: &Value) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

fn semantic(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, child) in map {
                if matches!(key.as_str(), "generated_at" | "fetched_at" | "updated_at") {
                    continue;
                }
                result.insert(key.clone(), semantic(child));
            }
            Value::Object(result)
        }
        Value::Array(values) => Value::Array(values.iter().map(semantic).collect()),
        _ => value.clone(),
    }
}
