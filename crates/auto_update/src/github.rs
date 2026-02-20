use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
}

fn detect_arch() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "arm64"
    }
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "unknown"
    }
}

pub fn fetch_latest_release() -> Result<ReleaseInfo> {
    let url = "https://api.github.com/repos/lassejlv/termy/releases/latest";
    let response: GithubRelease = ureq::get(url)
        .set("User-Agent", "Termy-Updater/1.0")
        .set("Accept", "application/vnd.github+json")
        .call()
        .context("Failed to fetch latest release from GitHub")?
        .into_json()
        .context("Failed to parse GitHub release JSON")?;

    let version = response
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&response.tag_name)
        .to_string();
    let arch = detect_arch();
    let dmg_suffix = format!("{}.dmg", arch);

    let asset = response
        .assets
        .iter()
        .find(|a| a.name.contains(arch) && a.name.ends_with(".dmg"))
        .or_else(|| {
            response
                .assets
                .iter()
                .find(|a| a.name.ends_with(&dmg_suffix))
        })
        .or_else(|| response.assets.iter().find(|a| a.name.ends_with(".dmg")))
        .context(format!("No DMG asset found for architecture '{}'", arch))?;

    Ok(ReleaseInfo {
        version,
        download_url: asset.browser_download_url.clone(),
    })
}
