mod config;
mod http;
mod model;
mod sources;
mod store;
mod timeparse;
mod util;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde_json::json;

use crate::config::LOCALES;
use crate::http::HttpClient;
use crate::sources::{bethesda, nukaknights};

#[derive(Debug, Parser)]
#[command(
    name = "overseer",
    version,
    about = "Fallout 76 public data synchronizer"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Fetch all configured sources and update JSON files.
    Sync {
        /// Destination directory for generated JSON.
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Sync { data_dir } => sync(&data_dir),
    }
}

fn sync(data_dir: &Path) -> Result<()> {
    let client = HttpClient::new()?;
    let generated_at = Utc::now();

    for locale in LOCALES {
        println!("syncing {}", locale.code);
        let locale_root = data_dir.join(locale.code);

        let minerva = nukaknights::fetch_minerva(&client, locale, generated_at)
            .with_context(|| format!("sync Minerva for {}", locale.code))?;

        let events = nukaknights::fetch_events(&client, locale, generated_at)
            .with_context(|| format!("sync events for {}", locale.code))?;

        let atomic = bethesda::fetch_atomic_shop(&client, locale, generated_at)
            .with_context(|| format!("sync Atomic Shop for {}", locale.code))?;

        /*
         * Minerva
         */

        let minerva_dir = locale_root.join("minerva");

        store::write_versioned(
            &minerva_dir.join("current.json"),
            &minerva_dir.join("archive"),
            &json!({
                "schema_version": 1,
                "generated_at": generated_at,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "source": minerva.source.clone(),
                "sale": minerva.current.clone(),
            }),
        )?;

        store::write_stable(
            &minerva_dir.join("next.json"),
            &json!({
                "schema_version": 1,
                "generated_at": generated_at,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "source": minerva.source.clone(),
                "sale": minerva.next.clone(),
            }),
        )?;

        store::write_stable(
            &minerva_dir.join("schedule.json"),
            &json!({
                "schema_version": 1,
                "generated_at": generated_at,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "source": minerva.source.clone(),
                "schedule": minerva.schedule.clone(),
            }),
        )?;

        /*
         * Events
         */

        let events_dir = locale_root.join("events");

        store::write_versioned(
            &events_dir.join("current.json"),
            &events_dir.join("archive"),
            &serde_json::to_value(events)?,
        )?;

        /*
         * Atomic Shop
         */

        let atomic_dir = locale_root.join("atomic-shop");

        // Only offers that are active right now.
        store::write_stable(
            &atomic_dir.join("current.json"),
            &json!({
                "schema_version": 1,
                "generated_at": generated_at,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "source": atomic.source.clone(),
                "offers": atomic.active.clone(),
            }),
        )?;

        // Everything already announced that starts in the future.
        let upcoming: Vec<_> = atomic
            .offers
            .iter()
            .filter(|offer| {
                if offer.active {
                    return false;
                }

                offer
                    .starts_at
                    .as_ref()
                    .map(|starts_at| starts_at > &generated_at)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        store::write_stable(
            &atomic_dir.join("upcoming.json"),
            &json!({
                "schema_version": 1,
                "generated_at": generated_at,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "source": atomic.source.clone(),
                "offers": upcoming,
            }),
        )?;

        // Full monthly Bethesda Atomic Shop schedule.
        let archive_path = atomic_dir.join("archive").join(format!(
            "{:04}-{:02}.json",
            atomic.article_year, atomic.article_month
        ));

        store::write_stable(&archive_path, &serde_json::to_value(&atomic)?)?;

        /*
         * Locale index
         */

        store::write_stable(
            &locale_root.join("index.json"),
            &json!({
                "schema_version": 1,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "endpoints": {
                    "minerva_current": "minerva/current.json",
                    "minerva_next": "minerva/next.json",
                    "minerva_schedule": "minerva/schedule.json",
                    "events": "events/current.json",
                    "atomic_shop_current": "atomic-shop/current.json",
                    "atomic_shop_upcoming": "atomic-shop/upcoming.json"
                }
            }),
        )?;
    }

    /*
     * Root index
     */

    store::write_stable(
        &data_dir.join("index.json"),
        &json!({
            "schema_version": 1,
            "project": "Overseer",
            "locales": [
                "de-DE",
                "en-US"
            ],
            "endpoints": {
                "de-DE": "de-DE/index.json",
                "en-US": "en-US/index.json"
            }
        }),
    )?;

    Ok(())
}
