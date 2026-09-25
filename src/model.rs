use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub provider: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinervaItem {
    pub name: String,
    pub price_gold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinervaSale {
    pub list: u32,
    pub big_sale: bool,
    pub location: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub starts_at_local: String,
    pub ends_at_local: String,
    pub active: bool,
    pub inventory_complete: bool,
    pub items: Vec<MinervaItem>,
}

#[derive(Debug, Clone)]
pub struct MinervaFeed {
    pub source: SourceRef,
    pub current: Option<MinervaSale>,
    pub next: Option<MinervaSale>,
    pub schedule: Vec<MinervaSale>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub title: String,
    pub url: String,
    pub tags: Vec<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub starts_at_local: String,
    pub ends_at_local: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub name: String,
    pub target: Option<u32>,
    pub score_reward: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengesFeed {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub locale: String,
    pub timezone: String,
    pub source: SourceRef,
    pub period_start: DateTime<Utc>,
    pub period_start_local: String,
    pub reset_at: DateTime<Utc>,
    pub reset_at_local: String,
    pub challenges: Vec<Challenge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyOpsFeed {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub locale: String,
    pub timezone: String,
    pub source: SourceRef,
    pub mode: String,
    pub location: String,
    pub enemies: String,
    pub mutations: Vec<String>,
    pub period_start: DateTime<Utc>,
    pub period_start_local: String,
    pub reset_at: DateTime<Utc>,
    pub reset_at_local: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NukeCodesFeed {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub locale: String,
    pub timezone: String,
    pub source: SourceRef,
    pub alpha: String,
    pub bravo: String,
    pub charlie: String,
    pub valid_from: DateTime<Utc>,
    pub valid_from_local: String,
    pub resets_at: DateTime<Utc>,
    pub resets_at_local: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonFeed {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub locale: String,
    pub timezone: String,
    pub source: SourceRef,
    pub season_number: u32,
    pub season_name: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub starts_at_local: Option<String>,
    pub ends_at: Option<DateTime<Utc>>,
    pub ends_at_local: Option<String>,
    pub end_estimated: bool,
    pub remaining_days: Option<u32>,
    pub total_days: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct NukaHomeFeed {
    pub html: String,
    pub daily_challenges: ChallengesFeed,
    pub weekly_challenges: ChallengesFeed,
    pub daily_ops: DailyOpsFeed,
    pub nuke_codes: NukeCodesFeed,
    pub season: SeasonFeed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsFeed {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub locale: String,
    pub timezone: String,
    pub source: SourceRef,
    pub active: Vec<Event>,
    pub upcoming: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicOffer {
    pub item: String,
    pub price_atoms: Option<u32>,
    pub price_text: String,
    pub discount_percent: Option<u32>,
    pub section: String,
    pub subsection: String,
    pub offer_type: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub starts_at_local: Option<String>,
    pub ends_at_local: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicShopFeed {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub locale: String,
    pub timezone: String,
    pub source: SourceRef,
    pub article_title: String,
    pub article_year: i32,
    pub article_month: u32,
    pub active: Vec<AtomicOffer>,
    pub offers: Vec<AtomicOffer>,
}
