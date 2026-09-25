use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::config::LocaleConfig;
use crate::http::HttpClient;
use crate::model::{Event, EventsFeed, MinervaFeed, MinervaItem, MinervaSale, SourceRef};
use crate::timeparse::{parse_nuka_moment, parse_nuka_range, to_local_string};
use crate::util::{flatten_text, normalize_ws};

pub fn fetch_minerva(
    client: &HttpClient,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<MinervaFeed> {
    let schedule_html = client.get_text(locale.nuka_minerva_url)?;
    let schedule_doc = Html::parse_document(&schedule_html);
    let mut schedule = parse_minerva_schedule(&schedule_doc, locale, now)?;

    if schedule.is_empty() {
        bail!("Nuka Knights parser returned no Minerva schedule; refusing to overwrite good data");
    }

    let home_html = client.get_text(locale.nuka_home_url)?;
    let home_doc = Html::parse_document(&home_html);
    let inventory = parse_minerva_inventory(&home_doc, locale);

    for sale in &mut schedule {
        if let Some(items) = inventory.get(&sale.list) {
            sale.items = items.clone();
            sale.inventory_complete = !sale.items.is_empty();
        }
    }

    schedule.sort_by_key(|sale| sale.starts_at);
    let current = schedule
        .iter()
        .find(|sale| sale.starts_at <= now && now < sale.ends_at)
        .cloned();
    let next = schedule.iter().find(|sale| sale.starts_at > now).cloned();

    Ok(MinervaFeed {
        source: SourceRef {
            provider: "Nuka Knights".to_string(),
            url: locale.nuka_minerva_url.to_string(),
        },
        current,
        next,
        schedule,
    })
}

fn parse_minerva_schedule(
    document: &Html,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<Vec<MinervaSale>> {
    let lines = flatten_text(document);
    let list_re = Regex::new(r"(?i)\b(?:List|Liste)\s*(\d+)\b")?;
    let mut dedupe: BTreeMap<(u32, DateTime<Utc>), MinervaSale> = BTreeMap::new();

    for (index, line) in lines.iter().enumerate() {
        if !line.to_lowercase().contains("minerva") {
            continue;
        }
        let Some(caps) = list_re.captures(line) else {
            continue;
        };
        let list: u32 = caps[1].parse()?;
        let big_sale = line.to_lowercase().contains("big sale")
            || line.to_lowercase().contains("sonderangebote");

        let window_end = (index + 18).min(lines.len());
        let window = &lines[index..window_end];
        let location = window
            .iter()
            .find_map(|candidate| {
                candidate
                    .strip_prefix("Location:")
                    .or_else(|| candidate.strip_prefix("Ort:"))
                    .map(normalize_ws)
            })
            .unwrap_or_default();
        let range = window
            .iter()
            .find_map(|candidate| parse_nuka_range(candidate, locale.code, locale.timezone));

        let Some((starts_at, ends_at)) = range else {
            continue;
        };
        if location.is_empty() {
            continue;
        }

        dedupe
            .entry((list, starts_at))
            .or_insert_with(|| MinervaSale {
                list,
                big_sale,
                location,
                starts_at,
                ends_at,
                starts_at_local: to_local_string(starts_at, locale.timezone),
                ends_at_local: to_local_string(ends_at, locale.timezone),
                active: starts_at <= now && now < ends_at,
                inventory_complete: false,
                items: Vec::new(),
            });
    }

    Ok(dedupe.into_values().collect())
}

fn parse_minerva_inventory(
    document: &Html,
    locale: &LocaleConfig,
) -> HashMap<u32, Vec<MinervaItem>> {
    let lines = flatten_text(document);
    let list_re = Regex::new(r"(?i)\b(?:List|Liste)\s*(\d+)\b").expect("valid regex");
    let price_re = Regex::new(r"(?i)^([0-9][0-9.,]*)\s*Gold$").expect("valid regex");
    let mut result: HashMap<u32, Vec<MinervaItem>> = HashMap::new();

    for (index, line) in lines.iter().enumerate() {
        if !line.to_lowercase().contains("minerva") {
            continue;
        }
        let Some(caps) = list_re.captures(line) else {
            continue;
        };
        let Ok(list) = caps[1].parse::<u32>() else {
            continue;
        };

        let mut items = Vec::new();
        let mut cursor = index + 1;
        let stop = (index + 120).min(lines.len());
        while cursor < stop {
            let candidate = &lines[cursor];
            if cursor > index + 4
                && candidate.to_lowercase().contains("minerva")
                && list_re.is_match(candidate)
            {
                break;
            }
            if candidate.starts_with("Plan:") || candidate.starts_with("Bauplan:") {
                let mut price = None;
                for price_line in lines.iter().take((cursor + 6).min(stop)).skip(cursor + 1) {
                    if let Some(price_caps) = price_re.captures(price_line) {
                        let digits = price_caps[1].replace(',', "").replace('.', "");
                        price = digits.parse::<u32>().ok();
                        break;
                    }
                }
                if let Some(price_gold) = price {
                    items.push(MinervaItem {
                        name: candidate.clone(),
                        price_gold,
                    });
                }
            }
            cursor += 1;
        }

        if !items.is_empty() {
            result.entry(list).or_insert(items);
        }
    }

    if result.is_empty() {
        eprintln!(
            "warning: no Minerva inventory found on {} homepage; schedule will still be published",
            locale.code
        );
    }

    result
}

pub fn fetch_events(
    client: &HttpClient,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<EventsFeed> {
    let html = client.get_text(locale.nuka_events_url)?;
    let document = Html::parse_document(&html);
    let events = parse_events(&document, locale, now)
        .with_context(|| format!("parse {} Nuka Knights event calendar", locale.code))?;

    if events.is_empty() {
        bail!("Nuka Knights parser returned no events; refusing to overwrite good data");
    }

    let mut active: Vec<Event> = events
        .iter()
        .filter(|event| event.active)
        .cloned()
        .collect();
    let mut upcoming: Vec<Event> = events
        .into_iter()
        .filter(|event| event.starts_at > now)
        .take(100)
        .collect();
    active.sort_by_key(|event| event.starts_at);
    upcoming.sort_by_key(|event| event.starts_at);

    Ok(EventsFeed {
        schema_version: 1,
        generated_at: now,
        locale: locale.code.to_string(),
        timezone: locale.timezone.name().to_string(),
        source: SourceRef {
            provider: "Nuka Knights".to_string(),
            url: locale.nuka_events_url.to_string(),
        },
        active,
        upcoming,
    })
}

fn parse_events(document: &Html, locale: &LocaleConfig, now: DateTime<Utc>) -> Result<Vec<Event>> {
    let selector =
        Selector::parse("a[href*='/event/'], a[href*='/events/']").expect("valid selector");
    let base = Url::parse(locale.nuka_events_url)?;
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for anchor in document.select(&selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let absolute = base.join(href).unwrap_or_else(|_| base.clone()).to_string();
        let text = normalize_ws(&anchor.text().collect::<Vec<_>>().join(" "));
        if text.is_empty() {
            continue;
        }
        let entry = grouped.entry(absolute).or_default();
        if !entry.contains(&text) {
            entry.push(text);
        }
    }

    let mut events = Vec::new();
    for (url, texts) in grouped {
        let range = texts
            .iter()
            .find_map(|text| parse_nuka_range(text, locale.code, locale.timezone));
        let moment = texts
            .iter()
            .find_map(|text| parse_nuka_moment(text, locale.code, locale.timezone));

        let (starts_at, ends_at) = if let Some((start, end)) = range {
            (start, Some(end))
        } else if let Some(start) = moment {
            (start, None)
        } else {
            continue;
        };

        let title = texts
            .iter()
            .filter(|text| parse_nuka_range(text, locale.code, locale.timezone).is_none())
            .filter(|text| parse_nuka_moment(text, locale.code, locale.timezone).is_none())
            .filter(|text| text.chars().any(char::is_alphabetic))
            .filter(|text| {
                let lower = text.to_lowercase();
                !lower.starts_with("starts in")
                    && !lower.starts_with("ends in")
                    && !lower.starts_with("startet in")
                    && !lower.starts_with("endet in")
            })
            .max_by_key(|text| text.len())
            .cloned();

        let Some(title) = title else {
            continue;
        };
        let active = starts_at <= now && ends_at.map(|end| now < end).unwrap_or(false);

        events.push(Event {
            title,
            url,
            starts_at,
            ends_at,
            starts_at_local: to_local_string(starts_at, locale.timezone),
            ends_at_local: ends_at.map(|end| to_local_string(end, locale.timezone)),
            active,
        });
    }

    events.sort_by_key(|event| event.starts_at);
    events.dedup_by(|left, right| left.url == right.url);
    Ok(events)
}
