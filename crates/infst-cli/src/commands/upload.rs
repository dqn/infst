//! Upload command for bulk uploading tracker data to the web service.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use super::login::load_credentials;

#[derive(Deserialize)]
struct MappingEntry {
    #[serde(rename = "infinitasTitle")]
    infinitas_title: String,
    difficulty: String,
}

#[derive(Serialize)]
struct LampEntry {
    #[serde(rename = "infinitasTitle")]
    infinitas_title: String,
    difficulty: String,
    lamp: String,
    #[serde(rename = "exScore")]
    ex_score: u32,
    #[serde(rename = "missCount")]
    miss_count: u32,
}

pub fn run(
    tracker_path: &str,
    mapping_path: &str,
    endpoint: Option<&str>,
    token: Option<&str>,
) -> Result<()> {
    // Resolve endpoint/token: args > credentials file
    let (resolved_endpoint, resolved_token) = resolve_credentials(endpoint, token)?;

    // Read title-mapping.json
    let mapping_content =
        fs::read_to_string(mapping_path).context("Failed to read title mapping file")?;
    let mapping: HashMap<String, Vec<MappingEntry>> =
        serde_json::from_str(&mapping_content).context("Failed to parse title mapping JSON")?;

    // Build lookup set: (infinitasTitle, difficulty) -> true
    let mut title_diff_set: HashMap<(String, String), bool> = HashMap::new();
    for entries in mapping.values() {
        for entry in entries {
            title_diff_set.insert(
                (entry.infinitas_title.clone(), entry.difficulty.clone()),
                true,
            );
        }
    }

    // Read tracker.tsv
    let tracker_content =
        fs::read_to_string(tracker_path).context("Failed to read tracker TSV file")?;
    let mut lines = tracker_content.lines();

    let header = lines.next().context("Tracker TSV is empty")?;
    let columns: Vec<&str> = header.split('\t').collect();

    // Find column indices for each difficulty
    let difficulty_columns = find_difficulty_columns(&columns);

    let mut entries: Vec<LampEntry> = Vec::new();

    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 2 {
            continue;
        }

        let title = fields.get(1).unwrap_or(&"").to_string();
        if title.is_empty() {
            continue;
        }

        for (diff_name, col_indices) in &difficulty_columns {
            // Check if this (title, difficulty) is in the mapping
            if !title_diff_set.contains_key(&(title.clone(), diff_name.clone())) {
                continue;
            }

            let lamp = fields.get(col_indices.lamp).unwrap_or(&"").to_string();
            let ex_score: u32 = fields
                .get(col_indices.ex_score)
                .unwrap_or(&"0")
                .parse()
                .unwrap_or(0);
            let miss_count: u32 = fields
                .get(col_indices.miss_count)
                .unwrap_or(&"0")
                .parse()
                .unwrap_or(0);

            // Skip NO PLAY entries
            if lamp == "NO PLAY" || lamp.is_empty() {
                continue;
            }

            entries.push(LampEntry {
                infinitas_title: title.clone(),
                difficulty: diff_name.clone(),
                lamp,
                ex_score,
                miss_count,
            });
        }
    }

    if entries.is_empty() {
        println!("No matching entries found to upload.");
        return Ok(());
    }

    println!("Uploading {} entries...", entries.len());

    // POST /api/lamps/bulk
    let url = format!("{}/api/lamps/bulk", resolved_endpoint.trim_end_matches('/'));
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build();
    let agent: ureq::Agent = config.into();

    let body = serde_json::json!({ "entries": entries });
    let response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", resolved_token))
        .send_json(&body)
        .context("Failed to upload data")?;

    println!("Upload complete (status: {})", response.status());
    println!("Uploaded {} entries.", entries.len());

    Ok(())
}

pub fn resolve_credentials(
    endpoint: Option<&str>,
    token: Option<&str>,
) -> Result<(String, String)> {
    let creds = load_credentials();

    let resolved_endpoint = match endpoint {
        Some(e) => e.to_string(),
        None => creds
            .as_ref()
            .map(|(e, _)| e.clone())
            .context("No endpoint specified. Use --endpoint, INFST_API_ENDPOINT env, or run `infst login` first.")?,
    };

    let resolved_token = match token {
        Some(t) => t.to_string(),
        None => creds.as_ref().map(|(_, t)| t.clone()).context(
            "No token specified. Use --token, INFST_API_TOKEN env, or run `infst login` first.",
        )?,
    };

    Ok((resolved_endpoint, resolved_token))
}

struct DifficultyColumns {
    lamp: usize,
    ex_score: usize,
    miss_count: usize,
}

fn find_difficulty_columns(columns: &[&str]) -> Vec<(String, DifficultyColumns)> {
    let difficulties = ["SPN", "SPH", "SPA", "SPL"];
    let mut result = Vec::new();

    for diff in &difficulties {
        let lamp_name = format!("{}_Lamp", diff);
        let ex_score_name = format!("{}_EXScore", diff);
        let miss_count_name = format!("{}_MissCount", diff);

        let lamp_idx = columns.iter().position(|c| *c == lamp_name);
        let ex_score_idx = columns.iter().position(|c| *c == ex_score_name);
        let miss_count_idx = columns.iter().position(|c| *c == miss_count_name);

        if let (Some(lamp), Some(ex_score), Some(miss_count)) =
            (lamp_idx, ex_score_idx, miss_count_idx)
        {
            result.push((
                diff.to_string(),
                DifficultyColumns {
                    lamp,
                    ex_score,
                    miss_count,
                },
            ));
        }
    }

    result
}
