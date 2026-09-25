use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::StatusCode;

pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!(
                "Overseer/",
                env!("CARGO_PKG_VERSION"),
                " (Fallout 76 community data synchronizer)"
            ))
            .timeout(Duration::from_secs(30))
            .build()
            .context("build HTTP client")?;

        Ok(Self { client })
    }

    pub fn get_text(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .with_context(|| format!("GET {url}"))?
            .error_for_status()
            .with_context(|| format!("HTTP error for {url}"))?;

        response
            .text()
            .with_context(|| format!("read response body from {url}"))
    }

    pub fn get_text_optional(&self, url: &str) -> Result<Option<String>> {
        let response = self
            .client
            .get(url)
            .send()
            .with_context(|| format!("GET {url}"))?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = response
            .error_for_status()
            .with_context(|| format!("HTTP error for {url}"))?;

        Ok(Some(response.text().with_context(|| {
            format!("read response body from {url}")
        })?))
    }
}
