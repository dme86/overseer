use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc, Weekday};
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::config::LocaleConfig;
use crate::http::HttpClient;
use crate::model::{
    Challenge, ChallengesFeed, DailyOpsFeed, Event, EventsFeed, MinervaFeed, MinervaItem,
    MinervaSale, NukaHomeFeed, NukeCodesFeed, SeasonFeed, SourceRef,
};
use crate::timeparse::{
    local_to_utc, month_number, parse_nuka_moment, parse_nuka_range, to_local_string,
};
use crate::util::{flatten_text, normalize_ws};

pub fn fetch_minerva(
    client: &HttpClient,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
    home_html: &str,
) -> Result<MinervaFeed> {
    let schedule_html = client.get_text(locale.nuka_minerva_url)?;
    let schedule_doc = Html::parse_document(&schedule_html);
    let mut schedule = parse_minerva_schedule(&schedule_doc, locale, now)?;

    if schedule.is_empty() {
        bail!("Nuka Knights parser returned no Minerva schedule; refusing to overwrite good data");
    }

    let home_doc = Html::parse_document(home_html);
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

pub fn fetch_home(
    client: &HttpClient,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<NukaHomeFeed> {
    let html = client.get_text(locale.nuka_home_url)?;
    let document = Html::parse_document(&html);
    let source = SourceRef {
        provider: "Nuka Knights".to_string(),
        url: locale.nuka_home_url.to_string(),
    };

    let daily_ops = parse_daily_ops(&document, locale, now, source.clone())?;
    let daily_challenges = parse_challenges(
        &document,
        ".mod_fallout76challenges_daily",
        locale,
        now,
        source.clone(),
        daily_ops.period_start,
        daily_ops.reset_at,
    )?;
    let (week_start, week_end) = challenge_week(daily_ops.period_start)?;
    let weekly_challenges = parse_challenges(
        &document,
        ".mod_fallout76challenges_weekly",
        locale,
        now,
        source.clone(),
        week_start,
        week_end,
    )?;
    let nuke_codes = parse_nuke_codes(&document, locale, now, source.clone())?;
    let season = parse_season(&document, locale, now, source)?;

    Ok(NukaHomeFeed {
        html,
        daily_challenges,
        weekly_challenges,
        daily_ops,
        nuke_codes,
        season,
    })
}

fn parse_challenges(
    document: &Html,
    root_selector: &str,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
    source: SourceRef,
    period_start: DateTime<Utc>,
    reset_at: DateTime<Utc>,
) -> Result<ChallengesFeed> {
    let root_selector = Selector::parse(root_selector).expect("valid selector");
    let item_selector = Selector::parse("li.list-group-item").expect("valid selector");
    let name_selector = Selector::parse(".col-9").expect("valid selector");
    let reward_selector = Selector::parse(".badge-secondary").expect("valid selector");
    let target_re = Regex::new(r"\(\s*([0-9][0-9.,]*)\s*\)\s*$")?;
    let rank_re = Regex::new(r"^\d+\S*\s+")?;
    let number_re = Regex::new(r"[0-9][0-9.,]*")?;
    let root = document
        .select(&root_selector)
        .next()
        .with_context(|| format!("find {root_selector:?}"))?;
    let mut challenges = Vec::new();

    for item in root.select(&item_selector) {
        let Some(name_element) = item.select(&name_selector).next() else {
            continue;
        };
        let mut pieces = Vec::new();
        for text in name_element.text() {
            let text = normalize_ws(text);
            if text.eq_ignore_ascii_case("tips") || text.eq_ignore_ascii_case("tipps") {
                break;
            }
            if !text.is_empty() {
                pieces.push(text);
            }
        }
        let raw_name = normalize_ws(&pieces.join(" "));
        let target = target_re
            .captures(&raw_name)
            .and_then(|caps| caps[1].replace([',', '.'], "").parse::<u32>().ok());
        let without_target = target_re.replace(&raw_name, "");
        let name = normalize_ws(&rank_re.replace(&without_target, ""));
        let score_reward = item
            .select(&reward_selector)
            .next()
            .map(|element| normalize_ws(&element.text().collect::<Vec<_>>().join(" ")))
            .and_then(|text| {
                number_re
                    .find(&text)
                    .map(|value| value.as_str().to_string())
            })
            .and_then(|value| value.replace([',', '.'], "").parse::<u32>().ok());

        if !name.is_empty() {
            if let Some(score_reward) = score_reward {
                challenges.push(Challenge {
                    name,
                    target,
                    score_reward,
                });
            }
        }
    }

    if challenges.is_empty() {
        bail!("Nuka Knights parser returned no challenges for {root_selector:?}; refusing to overwrite good data");
    }

    Ok(ChallengesFeed {
        schema_version: 1,
        generated_at: now,
        locale: locale.code.to_string(),
        timezone: locale.timezone.name().to_string(),
        source,
        period_start,
        period_start_local: to_local_string(period_start, locale.timezone),
        reset_at,
        reset_at_local: to_local_string(reset_at, locale.timezone),
        challenges,
    })
}

fn parse_daily_ops(
    document: &Html,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
    source: SourceRef,
) -> Result<DailyOpsFeed> {
    let root_selector = Selector::parse(".mod_dailyops_widget").expect("valid selector");
    let root = document
        .select(&root_selector)
        .next()
        .context("find Daily Ops widget")?;
    let small_selector = Selector::parse("h4 + small").expect("valid selector");
    let type_selector = Selector::parse(".dailyopstype .col-12").expect("valid selector");
    let columns_selector = Selector::parse(".row.p-2 > .col-6").expect("valid selector");
    let date_text = root
        .select(&small_selector)
        .next()
        .map(|element| normalize_ws(&element.text().collect::<Vec<_>>().join(" ")))
        .context("find Daily Ops period")?;
    let period_start = parse_numeric_local_datetime(&date_text, locale.timezone)
        .context("parse Daily Ops period")?;
    let reset_at = add_local_days(period_start, locale.timezone, 1)?;
    let type_values: Vec<String> = root
        .select(&type_selector)
        .map(|element| normalize_ws(&element.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty())
        .collect();
    let columns: Vec<String> = root
        .select(&columns_selector)
        .map(|element| normalize_ws(&element.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty())
        .collect();

    if type_values.len() < 2 || columns.len() < 4 {
        bail!("Nuka Knights parser returned incomplete Daily Ops; refusing to overwrite good data");
    }

    Ok(DailyOpsFeed {
        schema_version: 1,
        generated_at: now,
        locale: locale.code.to_string(),
        timezone: locale.timezone.name().to_string(),
        source,
        mode: type_values[0].clone(),
        location: columns[2].clone(),
        enemies: columns[3].clone(),
        mutations: vec![type_values[1].clone(), columns[1].clone()],
        period_start,
        period_start_local: to_local_string(period_start, locale.timezone),
        reset_at,
        reset_at_local: to_local_string(reset_at, locale.timezone),
    })
}

fn parse_nuke_codes(
    document: &Html,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
    source: SourceRef,
) -> Result<NukeCodesFeed> {
    let row_selector =
        Selector::parse(".nuke-codes .row.d-none.d-md-flex").expect("valid selector");
    let cell_selector = Selector::parse(":scope > div").expect("valid selector");
    let row = document
        .select(&row_selector)
        .next()
        .context("find nuke codes")?;
    let cells: Vec<String> = row
        .select(&cell_selector)
        .map(|element| normalize_ws(&element.text().collect::<Vec<_>>().join(" ")))
        .collect();
    if cells.len() < 4 {
        bail!(
            "Nuka Knights parser returned incomplete nuke codes; refusing to overwrite good data"
        );
    }
    let code = |value: &str| -> Result<String> {
        Regex::new(r"\b([0-9]{8})\b")?
            .captures(value)
            .map(|caps| caps[1].to_string())
            .context("parse nuke code")
    };
    let (valid_from, resets_at) = parse_nuke_period(&cells[0], locale)?;

    Ok(NukeCodesFeed {
        schema_version: 1,
        generated_at: now,
        locale: locale.code.to_string(),
        timezone: locale.timezone.name().to_string(),
        source,
        alpha: code(&cells[1])?,
        bravo: code(&cells[2])?,
        charlie: code(&cells[3])?,
        valid_from,
        valid_from_local: to_local_string(valid_from, locale.timezone),
        resets_at,
        resets_at_local: to_local_string(resets_at, locale.timezone),
    })
}

fn parse_season(
    document: &Html,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
    source: SourceRef,
) -> Result<SeasonFeed> {
    let root_selector = Selector::parse(".season_desc").expect("valid selector");
    let button_selector = Selector::parse("button[data-content]").expect("valid selector");
    let root = document
        .select(&root_selector)
        .next()
        .context("find season widget")?;
    let visible = normalize_ws(&root.text().collect::<Vec<_>>().join(" "));
    let season_re = Regex::new(r#"(?i)(?:Season|Saison)\s+(\d+)\s*:\s*[\"']([^\"']+)[\"']"#)?;
    let season_caps = season_re.captures(&visible).context("parse season name")?;
    let remaining_re = Regex::new(r"(?i)(?:Still|Noch)\s+(\d+)\s*/\s*(\d+)\s+(?:Days Left|Tage)")?;
    let remaining = remaining_re.captures(&visible);
    let content = root
        .select(&button_selector)
        .next()
        .and_then(|button| button.value().attr("data-content"))
        .unwrap_or_default();
    let starts_at = parse_labeled_datetime(content, &["Start"], locale.timezone);
    let ends_at = parse_labeled_datetime(content, &["End", "Ende"], locale.timezone);
    let lower = content.to_lowercase();

    Ok(SeasonFeed {
        schema_version: 1,
        generated_at: now,
        locale: locale.code.to_string(),
        timezone: locale.timezone.name().to_string(),
        source,
        season_number: season_caps[1].parse()?,
        season_name: season_caps[2].to_string(),
        starts_at,
        starts_at_local: starts_at.map(|value| to_local_string(value, locale.timezone)),
        ends_at,
        ends_at_local: ends_at.map(|value| to_local_string(value, locale.timezone)),
        end_estimated: lower.contains("estimated") || lower.contains("schätzung"),
        remaining_days: remaining
            .as_ref()
            .and_then(|caps| caps[1].parse::<u32>().ok()),
        total_days: remaining.and_then(|caps| caps[2].parse::<u32>().ok()),
    })
}

fn parse_numeric_local_datetime(text: &str, timezone: chrono_tz::Tz) -> Option<DateTime<Utc>> {
    let re = Regex::new(r"(\d{1,2})\.(\d{1,2})\.(\d{4})\s*\((\d{1,2}):(\d{2})\)").ok()?;
    let caps = re.captures(text)?;
    local_to_utc(
        timezone,
        caps[3].parse().ok()?,
        caps[2].parse().ok()?,
        caps[1].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
    )
}

fn parse_labeled_datetime(
    text: &str,
    labels: &[&str],
    timezone: chrono_tz::Tz,
) -> Option<DateTime<Utc>> {
    for label in labels {
        let de = Regex::new(&format!(
            r"(?i){}:\s*(\d{{1,2}})\.(\d{{1,2}})\.(\d{{4}})\s+(\d{{1,2}}):(\d{{2}})",
            regex::escape(label)
        ))
        .ok()?;
        if let Some(caps) = de.captures(text) {
            return local_to_utc(
                timezone,
                caps[3].parse().ok()?,
                caps[2].parse().ok()?,
                caps[1].parse().ok()?,
                caps[4].parse().ok()?,
                caps[5].parse().ok()?,
            );
        }
        let en = Regex::new(&format!(
            r"(?i){}:\s*(\d{{1,2}})(?:st|nd|rd|th)?\s+([A-Za-z]+)\s+(\d{{4}})\s+(\d{{1,2}}):(\d{{2}})",
            regex::escape(label)
        ))
        .ok()?;
        if let Some(caps) = en.captures(text) {
            return local_to_utc(
                timezone,
                caps[3].parse().ok()?,
                month_number(&caps[2])?,
                caps[1].parse().ok()?,
                caps[4].parse().ok()?,
                caps[5].parse().ok()?,
            );
        }
    }
    None
}

fn parse_nuke_period(text: &str, locale: &LocaleConfig) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    if locale.code == "de-DE" {
        let re = Regex::new(
            r"(\d{1,2})\.(\d{1,2})\.-(\d{1,2})\.(\d{1,2})\.(\d{4})\s*\((\d{1,2}):(\d{2})\)",
        )?;
        let caps = re.captures(text).context("parse German nuke-code period")?;
        let year: i32 = caps[5].parse()?;
        let end_month: u32 = caps[4].parse()?;
        let start_month: u32 = caps[2].parse()?;
        let start_year = if start_month > end_month {
            year - 1
        } else {
            year
        };
        let start = local_to_utc(
            locale.timezone,
            start_year,
            start_month,
            caps[1].parse()?,
            caps[6].parse()?,
            caps[7].parse()?,
        )
        .context("invalid nuke-code start")?;
        let end = local_to_utc(
            locale.timezone,
            year,
            end_month,
            caps[3].parse()?,
            caps[6].parse()?,
            caps[7].parse()?,
        )
        .context("invalid nuke-code reset")?;
        return Ok((start, end));
    }

    let re = Regex::new(
        r"(?i)(\d{1,2})(?:st|nd|rd|th)?\s+([A-Za-z]+)\s*-(\d{1,2})(?:st|nd|rd|th)?\s+([A-Za-z]+)\s+(\d{4})\s*\((\d{1,2}):(\d{2})\)",
    )?;
    let caps = re
        .captures(text)
        .context("parse English nuke-code period")?;
    let year: i32 = caps[5].parse()?;
    let start_month = month_number(&caps[2]).context("invalid start month")?;
    let end_month = month_number(&caps[4]).context("invalid end month")?;
    let start_year = if start_month > end_month {
        year - 1
    } else {
        year
    };
    let start = local_to_utc(
        locale.timezone,
        start_year,
        start_month,
        caps[1].parse()?,
        caps[6].parse()?,
        caps[7].parse()?,
    )
    .context("invalid nuke-code start")?;
    let end = local_to_utc(
        locale.timezone,
        year,
        end_month,
        caps[3].parse()?,
        caps[6].parse()?,
        caps[7].parse()?,
    )
    .context("invalid nuke-code reset")?;
    Ok((start, end))
}

fn add_local_days(
    value: DateTime<Utc>,
    timezone: chrono_tz::Tz,
    days: i64,
) -> Result<DateTime<Utc>> {
    let local = value.with_timezone(&timezone);
    let date = local.date_naive() + Duration::days(days);
    timezone
        .with_ymd_and_hms(
            date.year(),
            date.month(),
            date.day(),
            local.hour(),
            local.minute(),
            0,
        )
        .single()
        .map(|value| value.with_timezone(&Utc))
        .context("invalid local reset time")
}

fn challenge_week(daily_start: DateTime<Utc>) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let timezone = chrono_tz::America::New_York;
    let local = daily_start.with_timezone(&timezone);
    let days_since_tuesday = match local.weekday() {
        Weekday::Mon => 6,
        day => day.num_days_from_monday() - Weekday::Tue.num_days_from_monday(),
    };
    let date = local.date_naive() - Duration::days(days_since_tuesday.into());
    let start = timezone
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 12, 0, 0)
        .single()
        .map(|value| value.with_timezone(&Utc))
        .context("invalid weekly challenge start")?;
    let end = add_local_days(start, timezone, 7)?;
    Ok((start, end))
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
                        let digits = price_caps[1].replace([',', '.'], "");
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
                    && !lower.starts_with("start in")
                    && !lower.starts_with("ende in")
                    && !lower.starts_with("startet in")
                    && !lower.starts_with("endet in")
            })
            .max_by_key(|text| text.len())
            .cloned();

        let Some(title) = title else {
            continue;
        };
        let active = starts_at <= now && ends_at.map(|end| now < end).unwrap_or(false);
        let tags = event_tags(&title);

        events.push(Event {
            title,
            url,
            tags,
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

fn event_tags(title: &str) -> Vec<String> {
    let lower = title.to_lowercase();
    let mappings: &[(&str, &[&str])] = &[
        ("treasure_hunter", &["treasure hunter", "schatzsucher"]),
        (
            "double_score",
            &[
                "double score",
                "doppelte score",
                "doppelter score",
                "doppel-score",
            ],
        ),
        (
            "double_xp",
            &["double xp", "doppelte ep", "doppelter ep", "doppel ep"],
        ),
        (
            "mutated_events",
            &[
                "mutated public events",
                "mutierte öffentliche events",
                "mutierte events",
            ],
        ),
        (
            "double_mutations",
            &[
                "double mutations",
                "doppelte mutationen",
                "doppelmutationen",
            ],
        ),
        (
            "spooky_scorched",
            &["spooky scorched", "gruselige verbrannte"],
        ),
        ("fasnacht", &["fasnacht"]),
        ("minerva", &["minerva"]),
        ("maintenance", &["maintenance", "wartung"]),
        ("caps_a_plenty", &["caps-a-plenty", "reichlich kronkorken"]),
        ("gold_rush", &["gold rush", "goldrausch"]),
        ("legendary_sale", &["legendary sale", "legendäre angebote"]),
        (
            "holiday_scorched",
            &["holiday scorched", "feiertags verbrannte"],
        ),
        (
            "mothman_equinox",
            &["mothman equinox", "mottenmann äquinoktium"],
        ),
        ("meat_week", &["meat week", "fleischwoche"]),
        (
            "invaders_from_beyond",
            &["invaders from beyond", "invasoren aus dem all"],
        ),
        ("mischief_night", &["mischief night", "nacht des unfugs"]),
        ("anniversary", &["anniversary", "geburtstags event"]),
        ("seasonal_fish", &["seasonal fish", "saisonale fisch"]),
    ];
    let mut tags: Vec<String> = mappings
        .iter()
        .filter(|(_, needles)| needles.iter().any(|needle| lower.contains(needle)))
        .map(|(tag, _)| (*tag).to_string())
        .collect();
    let seasonal = [
        "fasnacht",
        "mothman equinox",
        "mottenmann äquinoktium",
        "meat week",
        "fleischwoche",
        "invaders from beyond",
        "invasoren aus dem all",
        "spooky scorched",
        "gruselige verbrannte",
        "holiday scorched",
        "festliche verbrannte",
        "mischief night",
        "nacht des unfugs",
        "seasonal event",
        "saisonales event",
    ];
    if seasonal.iter().any(|needle| lower.contains(needle))
        || tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "treasure_hunter"
                    | "spooky_scorched"
                    | "fasnacht"
                    | "holiday_scorched"
                    | "mothman_equinox"
                    | "meat_week"
                    | "invaders_from_beyond"
                    | "mischief_night"
                    | "seasonal_fish"
            )
        })
    {
        tags.push("seasonal_event".to_string());
    }
    tags.sort();
    tags.dedup();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(code: &str) -> &'static LocaleConfig {
        crate::config::LOCALES
            .iter()
            .find(|locale| locale.code == code)
            .unwrap()
    }

    #[test]
    fn parses_challenges_without_modal_tip_content() {
        let document = Html::parse_document(
            r#"
            <div class="mod_fallout76challenges_daily">
              <li class="list-group-item">
                <div class="col-9"><b>1st</b> Kill a Gulper (<b>3</b>)
                  <button>Tips</button><div class="modal">Challenge: wrong text</div>
                </div>
                <div><span class="badge badge-secondary">250</span></div>
              </li>
            </div>
            "#,
        );
        let start = Utc.with_ymd_and_hms(2026, 9, 24, 16, 0, 0).unwrap();
        let reset = Utc.with_ymd_and_hms(2026, 9, 25, 16, 0, 0).unwrap();
        let feed = parse_challenges(
            &document,
            ".mod_fallout76challenges_daily",
            locale("en-US"),
            start,
            SourceRef {
                provider: "Nuka Knights".to_string(),
                url: "https://example.com".to_string(),
            },
            start,
            reset,
        )
        .unwrap();

        assert_eq!(feed.challenges.len(), 1);
        assert_eq!(feed.challenges[0].name, "Kill a Gulper");
        assert_eq!(feed.challenges[0].target, Some(3));
        assert_eq!(feed.challenges[0].score_reward, 250);
    }

    #[test]
    fn parses_localized_nuke_periods_as_the_same_instant() {
        let en = parse_nuke_period(
            "Nuke Codes (Week: 24th Sep -1st Oct 2026 (20:00))",
            locale("en-US"),
        )
        .unwrap();
        let de = parse_nuke_period(
            "Atomraketen Startcodes (Woche: 25.09.-02.10.2026 (02:00))",
            locale("de-DE"),
        )
        .unwrap();

        assert_eq!(en, de);
    }

    #[test]
    fn classifies_compound_and_seasonal_events() {
        assert_eq!(
            event_tags("Seasonal Fish Run, Treasure Hunter and Double Mutations"),
            vec![
                "double_mutations",
                "seasonal_event",
                "seasonal_fish",
                "treasure_hunter"
            ]
        );
        assert_eq!(
            event_tags("Update / Wartung für Patch 70"),
            vec!["maintenance"]
        );
    }
}
