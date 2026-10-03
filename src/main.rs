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

        let home = nukaknights::fetch_home(&client, locale, generated_at)
            .with_context(|| format!("sync Nuka Knights homepage for {}", locale.code))?;

        let minerva = nukaknights::fetch_minerva(&client, locale, generated_at, &home.html)
            .with_context(|| format!("sync Minerva for {}", locale.code))?;

        let events = nukaknights::fetch_events(&client, locale, generated_at)
            .with_context(|| format!("sync events for {}", locale.code))?;

        let atomic = bethesda::fetch_atomic_shop(&client, locale, generated_at)
            .with_context(|| format!("sync Atomic Shop for {}", locale.code))?;

        /*
         * Homepage feeds and semantic-period archives
         */

        if let Some(daily_challenges) = &home.daily_challenges {
            store::write_period_archive(
                &locale_root.join("challenges/daily.json"),
                &locale_root.join("challenges/daily/archive"),
                daily_challenges.period_start,
                &serde_json::to_value(daily_challenges)?,
            )?;
        }
        if let Some(weekly_challenges) = &home.weekly_challenges {
            store::write_period_archive(
                &locale_root.join("challenges/weekly.json"),
                &locale_root.join("challenges/weekly/archive"),
                weekly_challenges.period_start,
                &serde_json::to_value(weekly_challenges)?,
            )?;
        }
        store::write_period_archive(
            &locale_root.join("daily-ops/current.json"),
            &locale_root.join("daily-ops/archive"),
            home.daily_ops.period_start,
            &serde_json::to_value(&home.daily_ops)?,
        )?;
        store::write_period_archive(
            &locale_root.join("nuke-codes/current.json"),
            &locale_root.join("nuke-codes/archive"),
            home.nuke_codes.valid_from,
            &serde_json::to_value(&home.nuke_codes)?,
        )?;

        let season_dir = locale_root.join("season");
        let season_value = serde_json::to_value(&home.season)?;
        store::write_semantic_archive(
            &season_dir
                .join("archive")
                .join(format!("{}.json", home.season.season_number)),
            &season_value,
        )?;
        store::write_stable(&season_dir.join("current.json"), &season_value)?;

        /*
         * Minerva
         */

        let minerva_dir = locale_root.join("minerva");

        store::write_snapshotted(
            &minerva_dir.join("current.json"),
            &minerva_dir.join("snapshots"),
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

        store::write_snapshotted(
            &events_dir.join("current.json"),
            &events_dir.join("snapshots"),
            &serde_json::to_value(events)?,
        )?;

        /*
         * Atomic Shop
         */

        let atomic_dir = locale_root.join("atomic-shop");

        // Offers available right now.
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

        // Offers already announced but not yet available.
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

        /*
         * Atomic Shop monthly archive
         *
         * The archive intentionally does not contain the dynamic "active"
         * state. It represents Bethesda's published monthly schedule only.
         */

        let archived_offers: Vec<_> = atomic
            .offers
            .iter()
            .map(|offer| {
                json!({
                    "item": offer.item.clone(),
                    "price_atoms": offer.price_atoms,
                    "price_text": offer.price_text.clone(),
                    "discount_percent": offer.discount_percent,
                    "section": offer.section.clone(),
                    "subsection": offer.subsection.clone(),
                    "offer_type": offer.offer_type.clone(),
                    "starts_at": offer.starts_at,
                    "ends_at": offer.ends_at,
                    "starts_at_local": offer.starts_at_local.clone(),
                    "ends_at_local": offer.ends_at_local.clone(),
                })
            })
            .collect();

        let archive_path = atomic_dir
            .join("archive")
            .join(format!("{:04}", atomic.article_year))
            .join(format!("{:02}.json", atomic.article_month));

        store::write_semantic_archive(
            &archive_path,
            &json!({
                "schema_version": 1,
                "generated_at": generated_at,
                "locale": locale.code,
                "timezone": locale.timezone.name(),
                "source": atomic.source.clone(),
                "article_title": atomic.article_title.clone(),
                "article_year": atomic.article_year,
                "article_month": atomic.article_month,
                "offers": archived_offers,
            }),
        )?;

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
                    "daily_challenges": "challenges/daily.json",
                    "weekly_challenges": "challenges/weekly.json",
                    "daily_ops": "daily-ops/current.json",
                    "nuke_codes": "nuke-codes/current.json",
                    "season": "season/current.json",
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn current_nuke_codes_match_across_locales() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |locale: &str| -> Value {
            let path = root
                .join("data")
                .join(locale)
                .join("nuke-codes/current.json");
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
        };
        let de = read("de-DE");
        let en = read("en-US");

        for key in ["alpha", "bravo", "charlie"] {
            assert_eq!(de.get(key), en.get(key), "mismatched {key}");
        }
    }
}
