use serde::Deserialize;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "lassejlv/termy";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

pub fn run() {
    println!("Checking for updates...");
    println!();

    match check_for_updates() {
        Ok(result) => match result {
            UpdateResult::UpToDate => {
                println!("You're up to date! (v{})", CURRENT_VERSION);
            }
            UpdateResult::UpdateAvailable { version, url } => {
                println!("Update available!");
                println!();
                println!("  Current version: v{}", CURRENT_VERSION);
                println!("  Latest version:  v{}", version);
                println!();
                println!("Download at: {}", url);
                println!();
                println!("Or update via the Termy app: Command Palette > Check for Updates");
            }
        },
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
        }
    }
}

enum UpdateResult {
    UpToDate,
    UpdateAvailable { version: String, url: String },
}

fn check_for_updates() -> Result<UpdateResult, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let response: GithubRelease = ureq::get(&url)
        .set("User-Agent", "Termy-CLI/1.0")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?
        .into_json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let latest_version = response
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&response.tag_name)
        .to_string();

    let current = semver::Version::parse(CURRENT_VERSION)
        .map_err(|e| format!("Invalid current version: {}", e))?;
    let latest = semver::Version::parse(&latest_version)
        .map_err(|e| format!("Invalid latest version: {}", e))?;

    if latest > current {
        Ok(UpdateResult::UpdateAvailable {
            version: latest_version,
            url: response.html_url,
        })
    } else {
        Ok(UpdateResult::UpToDate)
    }
}
