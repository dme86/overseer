use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Utc};
use serde_json::Value;

use crate::util::sanitize_snapshot_name;

pub fn write_snapshotted(current_path: &Path, snapshots_dir: &Path, value: &Value) -> Result<bool> {
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

        let snapshot_path = snapshot_path(snapshots_dir, &old);

        if let Some(parent) = snapshot_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }

        if !snapshot_path.exists() {
            fs::write(&snapshot_path, pretty(&old)?)
                .with_context(|| format!("snapshot {}", snapshot_path.display()))?;

            println!("snapshotted {}", snapshot_path.display());
        }
    }

    fs::write(current_path, pretty(value)?)
        .with_context(|| format!("write {}", current_path.display()))?;

    println!("updated {}", current_path.display());

    Ok(true)
}

pub fn write_period_archive(
    current_path: &Path,
    archive_dir: &Path,
    period_start: DateTime<Utc>,
    value: &Value,
) -> Result<()> {
    let relative_path = period_archive_relative_path(period_start);
    write_semantic_archive(&archive_dir.join(relative_path), value)?;
    write_stable(current_path, value)?;
    Ok(())
}

fn period_archive_relative_path(period_start: DateTime<Utc>) -> PathBuf {
    PathBuf::from(format!(
        "{:04}/{:02}/{:02}.json",
        period_start.year(),
        period_start.month(),
        period_start.day()
    ))
}

pub fn write_semantic_archive(path: &Path, value: &Value) -> Result<bool> {
    write_stable(path, value)
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

fn snapshot_path(snapshots_dir: &Path, value: &Value) -> PathBuf {
    let generated_at = value.get("generated_at").and_then(Value::as_str);

    if let Some(generated_at) = generated_at {
        if let Ok(timestamp) = DateTime::parse_from_rfc3339(generated_at) {
            return snapshots_dir
                .join(format!("{:04}", timestamp.year()))
                .join(format!("{:02}", timestamp.month()))
                .join(format!("{}.json", sanitize_snapshot_name(generated_at)));
        }

        return snapshots_dir
            .join("unknown")
            .join(format!("{}.json", sanitize_snapshot_name(generated_at)));
    }

    snapshots_dir.join("unknown").join("unknown.json")
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn period_archive_path_uses_utc_date() {
        let from_german_locale = chrono_tz::Europe::Berlin
            .with_ymd_and_hms(2026, 9, 25, 2, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let from_english_locale = chrono_tz::America::New_York
            .with_ymd_and_hms(2026, 9, 24, 20, 0, 0)
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(from_german_locale, from_english_locale);
        assert_eq!(
            period_archive_relative_path(from_german_locale),
            PathBuf::from("2026/09/25.json")
        );
        assert_eq!(
            period_archive_relative_path(from_english_locale),
            PathBuf::from("2026/09/25.json")
        );
    }

    #[test]
    fn snapshot_path_is_keyed_by_generated_at() {
        let path = snapshot_path(
            Path::new("events/snapshots"),
            &json!({ "generated_at": "2026-10-02T12:30:47.424591385Z" }),
        );

        assert_eq!(
            path,
            PathBuf::from("events/snapshots/2026/10/2026-10-02T12-30-47.424591385Z.json")
        );
    }

    #[test]
    fn changed_current_state_is_written_to_snapshots() {
        let root = std::env::temp_dir().join(format!(
            "overseer-store-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let current = root.join("events/current.json");
        let snapshots = root.join("events/snapshots");
        fs::create_dir_all(current.parent().unwrap()).unwrap();
        let old = json!({
            "generated_at": "2026-10-02T12:30:47Z",
            "events": ["old"]
        });
        fs::write(&current, pretty(&old).unwrap()).unwrap();
        let new = json!({
            "generated_at": "2026-10-03T12:30:47Z",
            "events": ["new"]
        });

        assert!(write_snapshotted(&current, &snapshots, &new).unwrap());
        assert!(snapshots
            .join("2026/10/2026-10-02T12-30-47Z.json")
            .is_file());
        assert!(!root.join("events/archive").exists());

        fs::remove_dir_all(root).unwrap();
    }
}
