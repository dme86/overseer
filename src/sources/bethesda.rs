use anyhow::{bail, Context, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::config::LocaleConfig;
use crate::http::HttpClient;
use crate::model::{AtomicOffer, AtomicShopFeed, SourceRef};
use crate::timeparse::{
    noon_eastern, parse_month_day, plus_days, to_local_string, week_start_from_heading,
};
use crate::util::normalize_ws;

type OfferWindow = (Option<DateTime<Utc>>, Option<DateTime<Utc>>, String);

pub fn fetch_atomic_shop(
    client: &HttpClient,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<AtomicShopFeed> {
    let (url, html, article_year, article_month) = discover_article(client, locale, now)?;
    let document = Html::parse_document(&html);
    let title_selector = Selector::parse("h1").expect("valid selector");
    let article_title = document
        .select(&title_selector)
        .next()
        .map(element_text)
        .unwrap_or_else(|| "Atomic Shop Monthly Update".to_string());

    let mut offers = parse_offers(&document, article_year, locale, now)
        .context("parse Bethesda Atomic Shop tables")?;
    if offers.is_empty() {
        bail!("Bethesda parser returned no Atomic Shop offers; refusing to overwrite good data");
    }
    offers.sort_by(|a, b| {
        a.starts_at
            .cmp(&b.starts_at)
            .then_with(|| a.item.cmp(&b.item))
    });
    let active = offers
        .iter()
        .filter(|offer| offer.active)
        .cloned()
        .collect();

    Ok(AtomicShopFeed {
        schema_version: 1,
        generated_at: now,
        locale: locale.code.to_string(),
        timezone: locale.timezone.name().to_string(),
        source: SourceRef {
            provider: "Bethesda".to_string(),
            url,
        },
        article_title,
        article_year,
        article_month,
        active,
        offers,
    })
}

fn discover_article(
    client: &HttpClient,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<(String, String, i32, u32)> {
    let mut year = now.year();
    let mut month = now.month();

    for _ in 0..3 {
        let slug = format!(
            "atomic-shop-monthly-update-{}-{}",
            month_name(month).to_ascii_lowercase(),
            year
        );
        let url = format!(
            "https://fallout.bethesda.net/{}/news/{}",
            locale.bethesda_locale, slug
        );
        if let Some(html) = client.get_text_optional(&url)? {
            return Ok((url, html, year, month));
        }

        if month == 1 {
            month = 12;
            year -= 1;
        } else {
            month -= 1;
        }
    }

    bail!(
        "could not discover a Bethesda Atomic Shop monthly update for the current/previous months"
    )
}

fn parse_offers(
    document: &Html,
    year: i32,
    locale: &LocaleConfig,
    now: DateTime<Utc>,
) -> Result<Vec<AtomicOffer>> {
    let selector = Selector::parse("h2, h3, table").expect("valid selector");
    let mut h2 = String::new();
    let mut h3 = String::new();
    let mut offers = Vec::new();

    for element in document.select(&selector) {
        match element.value().name() {
            "h2" => {
                h2 = element_text(element);
                h3.clear();
            }
            "h3" => h3 = element_text(element),
            "table" => {
                let rows = table_rows(element);
                if rows.len() < 2 {
                    continue;
                }
                let headers = &rows[0];
                let week_start = week_start_from_heading(&h2, year)
                    .or_else(|| week_start_from_heading(&h3, year));

                for row in rows.iter().skip(1) {
                    let item = cell(headers, row, |header| {
                        matches_lower(header, &["item", "gegenstand"])
                    });
                    let Some(item) = item else {
                        continue;
                    };
                    if item.is_empty() {
                        continue;
                    }

                    let price_text = cell(headers, row, |header| {
                        let h = header.to_lowercase();
                        h.contains("atom") || h.contains("price") || h.contains("preis")
                    })
                    .unwrap_or_default();
                    let available_on = cell(headers, row, |header| {
                        matches_lower(header, &["available on", "erhältlich am", "erhaeltlich am"])
                    });
                    let available_from = cell(headers, row, |header| {
                        matches_lower(
                            header,
                            &[
                                "available from",
                                "erhältlich ab",
                                "erhaeltlich ab",
                                "gültig ab",
                                "gueltig ab",
                            ],
                        )
                    });
                    let available_until = cell(headers, row, |header| {
                        matches_lower(
                            header,
                            &[
                                "available until",
                                "erhältlich bis",
                                "erhaeltlich bis",
                                "gültig bis",
                                "gueltig bis",
                            ],
                        )
                    });

                    let (starts_at, ends_at, offer_type) = derive_window(
                        year,
                        week_start,
                        available_on.as_deref(),
                        available_from.as_deref(),
                        available_until.as_deref(),
                    )?;
                    let active = starts_at.map(|start| start <= now).unwrap_or(false)
                        && ends_at.map(|end| now < end).unwrap_or(true);

                    offers.push(AtomicOffer {
                        item,
                        price_atoms: parse_atoms(&price_text),
                        price_text: price_text.clone(),
                        discount_percent: parse_discount(&price_text),
                        section: h2.clone(),
                        subsection: h3.clone(),
                        offer_type,
                        starts_at,
                        ends_at,
                        starts_at_local: starts_at
                            .map(|value| to_local_string(value, locale.timezone)),
                        ends_at_local: ends_at.map(|value| to_local_string(value, locale.timezone)),
                        active,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(offers)
}

fn derive_window(
    year: i32,
    week_start: Option<NaiveDate>,
    available_on: Option<&str>,
    available_from: Option<&str>,
    available_until: Option<&str>,
) -> Result<OfferWindow> {
    if let Some(on) = available_on.and_then(|value| parse_month_day(value, year)) {
        let start = noon_eastern(on)?;
        return Ok((Some(start), Some(plus_days(start, 1)), "daily".to_string()));
    }

    let mut start = available_from
        .and_then(|value| parse_month_day(value, year))
        .map(noon_eastern)
        .transpose()?;
    let mut end = available_until
        .and_then(|value| parse_month_day(value, year))
        .map(noon_eastern)
        .transpose()?;

    if start.is_none() {
        start = week_start.map(noon_eastern).transpose()?;
    }
    if end.is_none() {
        end = start.map(|value| value + Duration::days(7));
    }

    let offer_type = if available_from.is_some() || available_until.is_some() {
        "dated"
    } else if week_start.is_some() {
        "weekly"
    } else {
        "unscheduled"
    };

    Ok((start, end, offer_type.to_string()))
}

fn table_rows(table: ElementRef<'_>) -> Vec<Vec<String>> {
    let row_selector = Selector::parse("tr").expect("valid selector");
    let cell_selector = Selector::parse("th, td").expect("valid selector");
    table
        .select(&row_selector)
        .map(|row| {
            row.select(&cell_selector)
                .map(element_text)
                .collect::<Vec<_>>()
        })
        .filter(|row| !row.is_empty())
        .collect()
}

fn cell<F>(headers: &[String], row: &[String], predicate: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    headers
        .iter()
        .position(|header| predicate(header))
        .and_then(|index| row.get(index))
        .cloned()
}

fn matches_lower(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn element_text(element: ElementRef<'_>) -> String {
    normalize_ws(&element.text().collect::<Vec<_>>().join(" "))
}

fn parse_atoms(value: &str) -> Option<u32> {
    let re = Regex::new(r"\d[\d.,]*").ok()?;
    let matched = re.find(value)?.as_str().replace([',', '.'], "");
    matched.parse().ok()
}

fn parse_discount(value: &str) -> Option<u32> {
    let re = Regex::new(r"(\d+)\s*%").ok()?;
    re.captures(value)?.get(1)?.as_str().parse().ok()
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_atom_price_and_discount() {
        assert_eq!(parse_atoms("1,260 (30% off!)"), Some(1260));
        assert_eq!(parse_discount("1,260 (30% off!)"), Some(30));
    }

    #[test]
    fn parses_representative_table() {
        let html = r#"
            <h2>Week of September 22</h2>
            <h3>Weekly Offers</h3>
            <table>
              <tr><th>Item</th><th>Discounted Atoms Price</th><th>Available on</th></tr>
              <tr><td>Grognak Axe Paint</td><td>350 (30% off!)</td><td>September 24</td></tr>
            </table>
        "#;
        let document = Html::parse_document(html);
        let locale = crate::config::LOCALES
            .iter()
            .find(|l| l.code == "en-US")
            .unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T17:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let offers = parse_offers(&document, 2026, locale, now).unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].item, "Grognak Axe Paint");
        assert_eq!(offers[0].price_atoms, Some(350));
    }
}
