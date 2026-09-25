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
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub starts_at_local: String,
    pub ends_at_local: Option<String>,
    pub active: bool,
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
