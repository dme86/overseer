use chrono_tz::{America::New_York, Europe::Berlin, Tz};

#[derive(Debug, Clone, Copy)]
pub struct LocaleConfig {
    pub code: &'static str,
    pub timezone: Tz,
    pub nuka_home_url: &'static str,
    pub nuka_minerva_url: &'static str,
    pub nuka_events_url: &'static str,
    pub bethesda_locale: &'static str,
}

pub const LOCALES: &[LocaleConfig] = &[
    LocaleConfig {
        code: "de-DE",
        timezone: Berlin,
        nuka_home_url: "https://nukaknights.com/",
        nuka_minerva_url: "https://nukaknights.com/minerva.html",
        nuka_events_url: "https://nukaknights.com/events-kalender.html",
        bethesda_locale: "de-DE",
    },
    LocaleConfig {
        code: "en-US",
        timezone: New_York,
        nuka_home_url: "https://nukaknights.com/en/",
        nuka_minerva_url: "https://nukaknights.com/minerva-dates-inventory.html",
        nuka_events_url: "https://nukaknights.com/events-calendar.html",
        bethesda_locale: "en-US",
    },
];
