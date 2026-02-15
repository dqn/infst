//! Login command for device code authentication flow.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_url: String,
    #[allow(dead_code)]
    expires_in: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    status: String,
    token: Option<String>,
}

fn credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("Failed to determine config directory")?;
    Ok(config_dir.join("infst").join("credentials"))
}

pub fn run(endpoint: &str) -> Result<()> {
    let endpoint = endpoint.trim_end_matches('/');

    // Request device code
    let url = format!("{}/auth/device/code", endpoint);
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build();
    let agent: ureq::Agent = config.into();

    let mut resp = agent
        .post(&url)
        .send("")
        .context("Failed to request device code")?;
    let response: DeviceCodeResponse = resp
        .body_mut()
        .read_json()
        .context("Failed to parse device code response")?;

    println!("Please visit the following URL and enter the code:");
    println!();
    println!("  URL:  {}", response.verification_url);
    println!("  Code: {}", response.user_code);
    println!();

    // Open browser
    if let Err(e) = open::that(&response.verification_url) {
        eprintln!("Failed to open browser: {}", e);
        println!("Please open the URL manually.");
    }

    println!("Waiting for authorization...");

    // Poll for token
    let token_url = format!("{}/auth/device/token", endpoint);
    let max_attempts = 150; // 5 minutes at 2-second intervals
    for _ in 0..max_attempts {
        std::thread::sleep(Duration::from_secs(2));

        let body = serde_json::json!({
            "device_code": response.device_code,
        });

        let token_response: TokenResponse = match agent.post(&token_url).send_json(&body) {
            Ok(mut resp) => match resp.body_mut().read_json() {
                Ok(parsed) => parsed,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        match token_response.status.as_str() {
            "approved" => {
                let token = token_response
                    .token
                    .context("Token missing from approved response")?;

                // Save credentials
                let cred_path = credentials_path()?;
                if let Some(parent) = cred_path.parent() {
                    fs::create_dir_all(parent).context("Failed to create config directory")?;
                }

                let content = toml::to_string_pretty(&toml::toml! {
                    endpoint = endpoint
                    token = token
                })
                .context("Failed to serialize credentials")?;

                fs::write(&cred_path, content).context("Failed to write credentials file")?;

                println!("Login successful!");
                println!("Credentials saved to: {}", cred_path.display());
                return Ok(());
            }
            "pending" => continue,
            status => bail!("Unexpected status: {}", status),
        }
    }

    bail!("Authorization timed out. Please try again.")
}

/// Load credentials from the config file
pub fn load_credentials() -> Option<(String, String)> {
    let cred_path = credentials_path().ok()?;
    let content = fs::read_to_string(cred_path).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let endpoint = table.get("endpoint")?.as_str()?.to_string();
    let token = table.get("token")?.as_str()?.to_string();
    Some((endpoint, token))
}
