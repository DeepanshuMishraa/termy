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
    pub extension: String,
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

#[cfg(target_os = "macos")]
fn find_platform_asset<'a>(assets: &'a [GithubAsset], arch: &str) -> Option<&'a GithubAsset> {
    let dmg_suffix = format!("{}.dmg", arch);
    assets
        .iter()
        .find(|a| a.name.contains(arch) && a.name.ends_with(".dmg"))
        .or_else(|| assets.iter().find(|a| a.name.ends_with(&dmg_suffix)))
        .or_else(|| assets.iter().find(|a| a.name.ends_with(".dmg")))
}

#[cfg(target_os = "windows")]
fn find_platform_asset<'a>(assets: &'a [GithubAsset], arch: &str) -> Option<&'a GithubAsset> {
    // Map Rust arch names to Windows installer naming conventions
    let win_arch = match arch {
        "x86_64" => "x64",
        "arm64" => "arm64",
        _ => arch,
    };

    // Prefer .msi installer, fall back to .exe
    // Check both Rust arch name (x86_64) and Windows name (x64)
    assets
        .iter()
        .find(|a| (a.name.contains(arch) || a.name.contains(win_arch)) && a.name.ends_with(".msi"))
        .or_else(|| assets.iter().find(|a| a.name.ends_with(".msi")))
        .or_else(|| {
            assets
                .iter()
                .find(|a| (a.name.contains(arch) || a.name.contains(win_arch)) && a.name.ends_with(".exe"))
        })
        .or_else(|| assets.iter().find(|a| a.name.ends_with(".exe")))
}

#[cfg(target_os = "linux")]
fn find_platform_asset<'a>(assets: &'a [GithubAsset], arch: &str) -> Option<&'a GithubAsset> {
    // Map Rust arch names to Linux tarball naming conventions
    let linux_arch = match arch {
        "arm64" => "aarch64",
        _ => arch,
    };

    // Look for .tar.gz tarball with matching architecture
    assets
        .iter()
        .find(|a| {
            a.name.contains("linux")
                && (a.name.contains(arch) || a.name.contains(linux_arch))
                && a.name.ends_with(".tar.gz")
        })
        .or_else(|| {
            // Fall back to any Linux tarball
            assets
                .iter()
                .find(|a| a.name.contains("linux") && a.name.ends_with(".tar.gz"))
        })
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn find_platform_asset<'a>(_assets: &'a [GithubAsset], _arch: &str) -> Option<&'a GithubAsset> {
    None
}

fn get_extension(name: &str) -> String {
    if name.ends_with(".tar.gz") {
        "tar.gz".to_string()
    } else if name.ends_with(".dmg") {
        "dmg".to_string()
    } else if name.ends_with(".msi") {
        "msi".to_string()
    } else if name.ends_with(".exe") {
        "exe".to_string()
    } else {
        "bin".to_string()
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

    let asset = find_platform_asset(&response.assets, arch).context(format!(
        "No installer asset found for this platform (arch: '{}')",
        arch
    ))?;

    Ok(ReleaseInfo {
        version,
        download_url: asset.browser_download_url.clone(),
        extension: get_extension(&asset.name),
    })
}
