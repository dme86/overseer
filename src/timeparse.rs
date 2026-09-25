use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use regex::Regex;

use crate::util::normalize_ws;

pub fn to_local_string(value: DateTime<Utc>, timezone: Tz) -> String {
    value.with_timezone(&timezone).to_rfc3339()
}

pub fn parse_nuka_range(
    text: &str,
    locale: &str,
    timezone: Tz,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    if locale == "de-DE" {
        parse_de_range(text, timezone)
    } else {
        parse_en_range(text, timezone)
    }
}

pub fn parse_nuka_moment(text: &str, locale: &str, timezone: Tz) -> Option<DateTime<Utc>> {
    if locale == "de-DE" {
        parse_de_moment(text, timezone)
    } else {
        parse_en_moment(text, timezone)
    }
}

pub fn local_to_utc(
    timezone: Tz,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<DateTime<Utc>> {
    timezone
        .with_ymd_and_hms(year, month, day, hour, minute, 0)
        .single()
        .map(|value| value.with_timezone(&Utc))
}

fn parse_de_range(text: &str, timezone: Tz) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let re = Regex::new(
        r"(?x)(?:Mo|Di|Mi|Do|Fr|Sa|So),?\s*(\d{1,2})\.(\d{1,2})\.(\d{4})\s*\((\d{1,2}):(\d{2})\)\s*[-–]\s*(?:Mo|Di|Mi|Do|Fr|Sa|So),?\s*(\d{1,2})\.(\d{1,2})\.(\d{4})\s*\((\d{1,2}):(\d{2})\)",
    )
    .ok()?;

    let normalized = normalize_ws(text);
    let caps = re.captures(&normalized)?;

    let start = local_to_utc(
        timezone,
        caps[3].parse().ok()?,
        caps[2].parse().ok()?,
        caps[1].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
    )?;

    let end = local_to_utc(
        timezone,
        caps[8].parse().ok()?,
        caps[7].parse().ok()?,
        caps[6].parse().ok()?,
        caps[9].parse().ok()?,
        caps[10].parse().ok()?,
    )?;

    Some((start, end))
}

fn parse_de_moment(text: &str, timezone: Tz) -> Option<DateTime<Utc>> {
    let re = Regex::new(
        r"(?:Mo|Di|Mi|Do|Fr|Sa|So),?\s*(\d{1,2})\.(\d{1,2})\.(\d{4})\s*\((\d{1,2}):(\d{2})\)",
    )
    .ok()?;

    let normalized = normalize_ws(text);
    let caps = re.captures(&normalized)?;

    local_to_utc(
        timezone,
        caps[3].parse().ok()?,
        caps[2].parse().ok()?,
        caps[1].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
    )
}

fn parse_en_range(text: &str, timezone: Tz) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let re = Regex::new(
        r"(?ix)(?:Mo|Tu|We|Th|Fr|Sa|Su),?\s*(\d{1,2})(?:st|nd|rd|th)?\s+([A-Za-z]+)\s+(\d{4})\s*\((\d{1,2}):(\d{2})\)\s*[-–]\s*(?:Mo|Tu|We|Th|Fr|Sa|Su),?\s*(\d{1,2})(?:st|nd|rd|th)?\s+([A-Za-z]+)\s+(\d{4})\s*\((\d{1,2}):(\d{2})\)",
    )
    .ok()?;

    let normalized = normalize_ws(text);
    let caps = re.captures(&normalized)?;

    let start = local_to_utc(
        timezone,
        caps[3].parse().ok()?,
        month_number(&caps[2])?,
        caps[1].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
    )?;

    let end = local_to_utc(
        timezone,
        caps[8].parse().ok()?,
        month_number(&caps[7])?,
        caps[6].parse().ok()?,
        caps[9].parse().ok()?,
        caps[10].parse().ok()?,
    )?;

    Some((start, end))
}

fn parse_en_moment(text: &str, timezone: Tz) -> Option<DateTime<Utc>> {
    let re = Regex::new(
        r"(?ix)(?:Mo|Tu|We|Th|Fr|Sa|Su),?\s*(\d{1,2})(?:st|nd|rd|th)?\s+([A-Za-z]+)\s+(\d{4})\s*\((\d{1,2}):(\d{2})\)",
    )
    .ok()?;

    let normalized = normalize_ws(text);
    let caps = re.captures(&normalized)?;

    local_to_utc(
        timezone,
        caps[3].parse().ok()?,
        month_number(&caps[2])?,
        caps[1].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
    )
}

pub fn parse_month_day(text: &str, year: i32) -> Option<NaiveDate> {
    let cleaned = normalize_ws(text)
        .replace(',', "")
        .replace("st ", " ")
        .replace("nd ", " ")
        .replace("rd ", " ")
        .replace("th ", " ");

    let en = Regex::new(r"(?i)\b([A-Za-z]+)\s+(\d{1,2})\b").ok()?;

    if let Some(caps) = en.captures(&cleaned) {
        return NaiveDate::from_ymd_opt(year, month_number(&caps[1])?, caps[2].parse().ok()?);
    }

    let de = Regex::new(
        r"(?i)\b(\d{1,2})\.?\s+(Januar|Februar|März|Maerz|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember)\b",
    )
    .ok()?;

    if let Some(caps) = de.captures(&cleaned) {
        return NaiveDate::from_ymd_opt(year, month_number(&caps[2])?, caps[1].parse().ok()?);
    }

    None
}

pub fn noon_eastern(date: NaiveDate) -> Result<DateTime<Utc>> {
    let tz: Tz = chrono_tz::America::New_York;

    tz.with_ymd_and_hms(date.year(), date.month(), date.day(), 12, 0, 0)
        .single()
        .map(|value| value.with_timezone(&Utc))
        .ok_or_else(|| anyhow!("invalid America/New_York date {date}"))
}

pub fn week_start_from_heading(text: &str, year: i32) -> Option<NaiveDate> {
    let normalized = normalize_ws(text);

    if !(normalized.to_lowercase().contains("week of")
        || normalized.to_lowercase().contains("woche"))
    {
        return None;
    }

    parse_month_day(&normalized, year)
}

pub fn month_number(value: &str) -> Option<u32> {
    match value.trim().to_ascii_lowercase().as_str() {
        "jan" | "january" | "januar" => Some(1),
        "feb" | "february" | "februar" => Some(2),
        "mar" | "march" | "märz" | "maerz" => Some(3),
        "apr" | "april" => Some(4),
        "may" | "mai" => Some(5),
        "jun" | "june" | "juni" => Some(6),
        "jul" | "july" | "juli" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" | "oktober" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" | "dezember" => Some(12),
        _ => None,
    }
}

pub fn plus_days(value: DateTime<Utc>, days: i64) -> DateTime<Utc> {
    value + Duration::days(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_german_nuka_range() {
        let (start, end) = parse_nuka_range(
            "Mo, 28.09.2026 (18:00) - Mi, 30.09.2026 (18:00)",
            "de-DE",
            chrono_tz::Europe::Berlin,
        )
        .unwrap();

        assert!(end > start);
    }

    #[test]
    fn parses_english_nuka_range() {
        let (start, end) = parse_nuka_range(
            "Mo, 28th Sep 2026 (12:00) - We, 30th Sep 2026 (12:00)",
            "en-US",
            chrono_tz::America::New_York,
        )
        .unwrap();

        assert!(end > start);
    }

    #[test]
    fn parses_atomic_month_day() {
        assert_eq!(parse_month_day("September 28", 2026).unwrap().day(), 28);

        assert_eq!(parse_month_day("28. September", 2026).unwrap().day(), 28);
    }
}
