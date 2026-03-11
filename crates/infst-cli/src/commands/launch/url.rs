//! URL parsing for bm2dxinf:// protocol handler.

use anyhow::{Result, bail};

/// Parsed launch URL data.
#[derive(Debug, PartialEq)]
pub struct LaunchParams {
    pub token: String,
    pub trial: bool,
}

/// Parse a `bm2dxinf://` URL and extract the launch token and trial flag.
///
/// The URL format is: `bm2dxinf://...?tk=<64-char-hex>[&trial=...]`
pub fn parse_launch_url(url: &str) -> Result<LaunchParams> {
    if !url.starts_with("bm2dxinf://") {
        bail!("Not a bm2dxinf:// URL: {url}");
    }

    let token = extract_token(url)?;
    let trial = url.contains("trial");

    Ok(LaunchParams { token, trial })
}

/// Extract the 64-character hex token from a `tk=` query parameter.
fn extract_token(url: &str) -> Result<String> {
    let tk_start = url
        .find("tk=")
        .map(|pos| pos + 3)
        .ok_or_else(|| anyhow::anyhow!("No tk= parameter found in URL: {url}"))?;

    let token: String = url[tk_start..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();

    if token.len() != 64 {
        bail!(
            "Token must be 64 hex characters, got {} characters: {token}",
            token.len()
        );
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normal_url() {
        let url = "bm2dxinf://some/path?tk=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let params = parse_launch_url(url).unwrap();
        assert_eq!(
            params.token,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert!(!params.trial);
    }

    #[test]
    fn parse_trial_url() {
        let url = "bm2dxinf://trial/path?tk=aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";
        let params = parse_launch_url(url).unwrap();
        assert_eq!(
            params.token,
            "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111"
        );
        assert!(params.trial);
    }

    #[test]
    fn parse_url_with_extra_params() {
        let url = "bm2dxinf://launch?tk=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&other=value";
        let params = parse_launch_url(url).unwrap();
        assert_eq!(params.token.len(), 64);
        assert!(!params.trial);
    }

    #[test]
    fn reject_non_bm2dxinf_url() {
        let result = parse_launch_url("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn reject_missing_token() {
        let result = parse_launch_url("bm2dxinf://path/without/token");
        assert!(result.is_err());
    }

    #[test]
    fn reject_short_token() {
        let result = parse_launch_url("bm2dxinf://path?tk=0123456789abcdef");
        assert!(result.is_err());
    }

    #[test]
    fn reject_non_hex_token() {
        let result = parse_launch_url(
            "bm2dxinf://path?tk=zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        );
        assert!(result.is_err());
    }

    #[test]
    fn uppercase_hex_token() {
        let url =
            "bm2dxinf://path?tk=0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";
        let params = parse_launch_url(url).unwrap();
        assert_eq!(params.token.len(), 64);
    }
}
