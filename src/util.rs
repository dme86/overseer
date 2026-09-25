use scraper::{Html, Selector};

pub fn normalize_ws(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn flatten_text(document: &Html) -> Vec<String> {
    let selector = Selector::parse("body").expect("valid body selector");
    let Some(body) = document.select(&selector).next() else {
        return Vec::new();
    };

    body.text()
        .map(normalize_ws)
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn sanitize_snapshot_name(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            ':' | '/' | '\\' | ' ' | '+' => '-',
            _ => c,
        })
        .collect()
}
