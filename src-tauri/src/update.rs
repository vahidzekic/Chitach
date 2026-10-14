//! Check GitHub releases for a newer version.

use serde::{Deserialize, Serialize};

use crate::error::{with_detail, CmdResult};

pub const RELEASES_API: &str = "https://api.github.com/repos/vahidzekic/Chitach/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/vahidzekic/Chitach/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    html_url: String,
}

fn parse_version(v: &str) -> Option<semver::Version> {
    let v = v.trim();
    let v = v.strip_prefix(['v', 'V']).unwrap_or(v);
    semver::Version::parse(v).ok()
}

/// Is `latest` a newer version than `current`? Unparseable versions are never newer.
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

fn info_from(current: &str, body: &str) -> CmdResult<UpdateInfo> {
    let rel: Release = serde_json::from_str(body).map_err(|e| with_detail("error.update", e))?;
    let latest = rel
        .tag_name
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();
    Ok(UpdateInfo {
        current: current.to_string(),
        newer: is_newer(current, &latest),
        latest,
        url: if rel.html_url.is_empty() {
            RELEASES_PAGE.to_string()
        } else {
            rel.html_url
        },
    })
}

/// Query GitHub (blocking).
pub fn check(current: &str) -> CmdResult<UpdateInfo> {
    let mut rsp = ureq::get(RELEASES_API)
        .header("User-Agent", concat!("Chitach/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| with_detail("error.update", e))?;
    let body = rsp
        .body_mut()
        .read_to_string()
        .map_err(|e| with_detail("error.update", e))?;
    info_from(current, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare() {
        assert!(is_newer("0.1.0", "v0.2.0"));
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(!is_newer("0.2.0", "v0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(is_newer("0.1.0-beta.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "garbage"));
    }

    #[test]
    fn parse_release() {
        let i = info_from(
            "0.1.0",
            r#"{"tag_name":"v0.3.0","html_url":"https://github.com/vahidzekic/Chitach/releases/tag/v0.3.0","x":1}"#,
        )
        .unwrap();
        assert_eq!(i.latest, "0.3.0");
        assert!(i.newer);
        assert!(i.url.ends_with("v0.3.0"));
    }
}
