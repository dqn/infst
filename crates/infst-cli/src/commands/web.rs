//! Open the infst web interface in the default browser.

use anyhow::{Context, Result};

use super::login::load_credentials;

const DEFAULT_ENDPOINT: &str = "https://infst.oidehosp.me";

pub fn run(endpoint: Option<&str>) -> Result<()> {
    let url = match endpoint {
        Some(ep) => ep.to_string(),
        None => load_credentials()
            .map(|(ep, _)| ep)
            .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
    };

    println!("Opening {}...", url);
    open::that(&url).context("Failed to open browser")?;
    Ok(())
}
