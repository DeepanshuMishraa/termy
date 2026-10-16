use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
pub use termy_core::release_core::ReleaseInfo;
pub use termy_core::release_core::{UpdateCheck, check_for_updates};

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        asset_name: String,
        url: String,
        checksum_asset_name: Option<String>,
        checksum_url: Option<String>,
        extension: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: u64,
    },
    Downloaded {
        version: String,
        installer_path: PathBuf,
    },
    Installing {
        version: String,
    },
    InstallerLaunched {
        version: String,
    },
    Installed {
        version: String,
    },
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InstallOutcome {
    Installed,
    InstallerLaunched,
}

#[cfg(target_os = "macos")]
pub fn cache_installer_path(version: &str, extension: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let cache_dir = PathBuf::from(home).join("Library/Caches/Termy");
    let _ = std::fs::create_dir_all(&cache_dir);
    cache_dir.join(format!("update-{version}.{extension}"))
}

#[cfg(target_os = "windows")]
pub fn cache_installer_path(version: &str, extension: &str) -> PathBuf {
    let cache_dir = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("Termy")
        .join("Cache");
    let _ = std::fs::create_dir_all(&cache_dir);
    cache_dir.join(format!("update-{}.{}", version, extension))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn cache_installer_path(version: &str, extension: &str) -> PathBuf {
    let cache_dir = std::env::temp_dir().join("termy-updates");
    let _ = std::fs::create_dir_all(&cache_dir);
    cache_dir.join(format!("update-{version}.{extension}"))
}

pub fn download_installer(
    url: &str,
    dest: &Path,
    progress_tx: flume::Sender<(u64, u64)>,
) -> Result<PathBuf> {
    let response = ureq::get(url)
        .set("User-Agent", "Termy-Updater/1.0")
        .call()
        .context("Failed to download installer")?;

    let total: u64 = response
        .header("Content-Length")
        .and_then(|h| h.parse().ok())
        .unwrap_or(0);

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(dest).context("Failed to create installer file")?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 65536]; // 64KiB chunks

    loop {
        let n = reader
            .read(&mut buf)
            .context("Failed to read download stream")?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        downloaded += n as u64;
        let _ = progress_tx.try_send((downloaded, total));
    }

    Ok(dest.to_path_buf())
}

pub fn verify_installer_checksum(
    installer_path: &Path,
    asset_name: &str,
    checksum_asset_name: Option<&str>,
    checksum_url: Option<&str>,
) -> Result<()> {
    // Every release publishes checksums.txt (see .github/workflows/release.yml),
    // so a missing checksum means the release is incomplete or tampered with.
    let Some(checksum_url) = checksum_url else {
        anyhow::bail!("Release is missing a checksum asset for {asset_name}");
    };

    let checksum_text = download_checksum_text(checksum_url)?;
    let allow_hash_only = checksum_asset_name.is_some_and(|checksum_asset_name| {
        checksum_asset_name.eq_ignore_ascii_case(&format!("{asset_name}.sha256"))
    });
    let expected = expected_sha256_for_asset(&checksum_text, asset_name, allow_hash_only)
        .with_context(|| format!("Checksum file did not contain an entry for {asset_name}"))?;
    let actual = file_sha256_hex(installer_path)?;

    if !actual.eq_ignore_ascii_case(&expected) {
        anyhow::bail!("Checksum mismatch for {asset_name}: expected {expected}, got {actual}");
    }

    Ok(())
}

fn download_checksum_text(url: &str) -> Result<String> {
    ureq::get(url)
        .set("User-Agent", "Termy-Updater/1.0")
        .call()
        .context("Failed to download checksum file")?
        .into_string()
        .context("Failed to read checksum file")
}

fn expected_sha256_for_asset(
    checksum_text: &str,
    asset_name: &str,
    allow_hash_only: bool,
) -> Option<String> {
    let mut only_hash = None;
    let mut hash_count = 0usize;

    for line in checksum_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let hashes = line
            .split_whitespace()
            .filter(|part| is_sha256_hex(part.trim()))
            .collect::<Vec<_>>();
        for hash in hashes {
            hash_count += 1;
            only_hash = Some(hash.to_ascii_lowercase());
            if checksum_line_matches_asset(line, asset_name) {
                return Some(hash.to_ascii_lowercase());
            }
        }
    }

    (allow_hash_only && hash_count == 1).then_some(only_hash?)
}

fn checksum_line_matches_asset(line: &str, asset_name: &str) -> bool {
    let normalized_asset = normalize_checksum_name(asset_name);
    line.split_whitespace()
        .skip(1)
        .map(normalize_checksum_name)
        .any(|part| part == normalized_asset)
}

fn normalize_checksum_name(name: &str) -> String {
    name.trim_matches(|ch| ch == '*' || ch == '"' || ch == '\'')
        .trim_start_matches("./")
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn file_sha256_hex(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).context("Failed to open downloaded installer")?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];

    loop {
        let n = file
            .read(&mut buf)
            .context("Failed to read downloaded installer for checksum")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Fails early, before anything is downloaded, when this build cannot replace
/// itself (for example an unsigned development build on macOS).
#[cfg(target_os = "macos")]
pub fn ensure_install_supported() -> Result<()> {
    running_signed_bundle().map(|_| ())
}

#[cfg(not(target_os = "macos"))]
#[allow(clippy::unnecessary_wraps)] // Same signature as the macOS check.
pub fn ensure_install_supported() -> Result<()> {
    Ok(())
}

/// The running `.app` bundle and the Team ID it is signed with.
#[cfg(target_os = "macos")]
fn running_signed_bundle() -> Result<(PathBuf, String)> {
    let exe = std::env::current_exe().context("Failed to resolve the running executable")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let bundle = app_bundle_for_executable(&exe)
        .context("Termy is not running from an app bundle; download the update manually instead")?;
    let team_id = codesign_team_identifier(&bundle)?.with_context(|| {
        format!(
            "{} is not signed with a Developer ID; download the update manually instead",
            bundle.display()
        )
    })?;
    Ok((bundle, team_id))
}

#[cfg(target_os = "macos")]
fn codesign_team_identifier(app: &Path) -> Result<Option<String>> {
    let output = std::process::Command::new("codesign")
        .args(["-dv", "--verbose=2"])
        .arg(app)
        .output()
        .context("Failed to run codesign")?;
    if !output.status.success() {
        return Ok(None);
    }
    // codesign prints the signature details on stderr.
    Ok(team_identifier_from_codesign_output(
        &String::from_utf8_lossy(&output.stderr),
    ))
}

/// Requires a valid, Apple-anchored signature from the same team as the
/// running app before the downloaded bundle may replace it.
#[cfg(target_os = "macos")]
fn verify_app_signature(app: &Path, expected_team_id: &str) -> Result<()> {
    // A leading `=` makes codesign read the requirement as text, not a path.
    let requirement = format!(
        "-R=anchor apple generic and certificate leaf[subject.OU] = \"{expected_team_id}\""
    );
    let output = std::process::Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&requirement)
        .arg(app)
        .output()
        .context("Failed to run codesign")?;
    if !output.status.success() {
        anyhow::bail!(
            "The downloaded app failed code signature verification: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let team_id = codesign_team_identifier(app)?;
    if team_id.as_deref() != Some(expected_team_id) {
        anyhow::bail!(
            "The downloaded app is signed by team {}, expected {expected_team_id}",
            team_id.as_deref().unwrap_or("(none)")
        );
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn team_identifier_from_codesign_output(output: &str) -> Option<String> {
    output
        .lines()
        .find_map(|line| line.trim().strip_prefix("TeamIdentifier="))
        .map(str::trim)
        // Team IDs are short alphanumeric strings; anything else (such as
        // "not set" for ad-hoc signatures) means there is no team.
        .filter(|team| !team.is_empty() && team.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .map(ToOwned::to_owned)
}

/// `/Applications/Termy.app/Contents/MacOS/termy` -> `/Applications/Termy.app`.
#[cfg(any(target_os = "macos", test))]
fn app_bundle_for_executable(exe: &Path) -> Option<PathBuf> {
    let macos_dir = exe.parent()?;
    let contents_dir = macos_dir.parent()?;
    let bundle = contents_dir.parent()?;
    let is_bundle = macos_dir.file_name()? == "MacOS"
        && contents_dir.file_name()? == "Contents"
        && bundle
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"));
    is_bundle.then(|| bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
pub fn do_install(dmg_path: &PathBuf) -> Result<InstallOutcome> {
    use std::process::Command;

    // Replace the bundle that is actually running, wherever it lives (e.g.
    // ~/Applications), and only with an app signed by the same team.
    let (running_bundle, team_id) = running_signed_bundle()?;
    let install_dir = running_bundle
        .parent()
        .context("Running app bundle has no parent directory")?
        .to_path_buf();
    let bundle_name = running_bundle
        .file_name()
        .and_then(|name| name.to_str())
        .context("Running app bundle is missing a valid file name")?
        .to_string();

    // Mount the DMG
    let mount = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly"])
        .arg(dmg_path)
        .output()
        .context("Failed to mount DMG")?;

    if !mount.status.success() {
        anyhow::bail!(
            "hdiutil attach failed: {}",
            String::from_utf8_lossy(&mount.stderr)
        );
    }

    let mount_stdout = String::from_utf8_lossy(&mount.stdout);
    let mount_point = mount_stdout
        .lines()
        .find_map(|line| {
            line.find("/Volumes/")
                .map(|start| PathBuf::from(line[start..].trim()))
        })
        .context(format!(
            "Could not determine mounted volume from hdiutil output: {}",
            mount_stdout.trim()
        ))?;

    let install_result: Result<()> = (|| {
        let mut app_path = None;
        for entry in std::fs::read_dir(&mount_point).context("Failed to read mounted volume")? {
            let entry = entry?;
            let path = entry.path();
            let is_app = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("app"));
            if !is_app {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some("Termy.app") {
                app_path = Some(path);
                break;
            }
            if app_path.is_none() {
                app_path = Some(path);
            }
        }

        let app_path = app_path.context("No .app bundle found inside mounted DMG")?;
        verify_app_signature(&app_path, &team_id)?;
        super::staged_install::install_staged(&install_dir, &[bundle_name.as_str()], |stage| {
            // ditto preserves bundle metadata; only a complete copy is published.
            let copy_result = Command::new("ditto")
                .arg(&app_path)
                .arg(stage.join(&bundle_name))
                .output()
                .with_context(|| {
                    format!("Failed to stage app bundle in {}", install_dir.display())
                })?;
            if !copy_result.status.success() {
                anyhow::bail!(
                    "ditto failed: {}",
                    String::from_utf8_lossy(&copy_result.stderr)
                );
            }
            // Verify the copy too, so nothing changed between check and install.
            verify_app_signature(&stage.join(&bundle_name), &team_id)
        })
    })();

    // Always try to detach, even if install failed.
    let _ = Command::new("hdiutil")
        .arg("detach")
        .arg(&mount_point)
        .arg("-quiet")
        .output();

    install_result.map(|()| InstallOutcome::Installed)
}

#[cfg(target_os = "windows")]
pub fn do_install(installer_path: &PathBuf) -> Result<InstallOutcome> {
    let extension = installer_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "msi" => {
            shell_execute_elevated(
                "msiexec.exe",
                &windows_msi_installer_parameters(installer_path),
            )
            .context("Failed to launch MSI installer")?;
        }
        "exe" => {
            shell_execute_elevated(
                &installer_path.to_string_lossy(),
                &windows_exe_installer_parameters(),
            )
            .context("Failed to launch EXE installer")?;
        }
        _ => {
            anyhow::bail!("Unsupported installer format: {}", extension);
        }
    }

    Ok(InstallOutcome::InstallerLaunched)
}

#[cfg(target_os = "windows")]
fn shell_execute_elevated(file: &str, parameters: &str) -> Result<()> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let operation = wide_null("runas");
    let file = wide_null(file);
    let parameters = wide_null(parameters);

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::from_raw(operation.as_ptr()),
            PCWSTR::from_raw(file.as_ptr()),
            PCWSTR::from_raw(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        anyhow::bail!("ShellExecuteW failed with code {code}");
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_msi_installer_parameters(installer_path: &Path) -> String {
    format!(
        "/i {} /passive /norestart",
        quote_windows_arg(&installer_path.to_string_lossy())
    )
}

#[cfg(target_os = "windows")]
fn windows_exe_installer_parameters() -> String {
    [
        "/SILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/CLOSEAPPLICATIONS",
    ]
    .join(" ")
}

#[cfg(target_os = "windows")]
fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty()
        && !arg
            .bytes()
            .any(|byte| byte == b' ' || byte == b'\t' || byte == b'"')
    {
        return arg.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;

    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', (backslashes * 2) + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                quoted.push(ch);
            }
        }
    }

    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn do_install(_installer_path: &PathBuf) -> Result<InstallOutcome> {
    anyhow::bail!("Auto-install is only supported on macOS and Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_parser_finds_matching_manifest_entry() {
        let checksums = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  Termy-v1.2.3-macos-arm64.dmg
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *Termy-v1.2.3-windows-x64-Setup.exe
";

        assert_eq!(
            expected_sha256_for_asset(checksums, "Termy-v1.2.3-windows-x64-Setup.exe", false),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string())
        );
    }

    #[test]
    fn checksum_parser_accepts_single_hash_asset_file_when_allowed() {
        assert_eq!(
            expected_sha256_for_asset(
                "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                "Termy-v1.2.3-windows-x64-Setup.exe",
                true,
            ),
            Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string())
        );
    }

    #[test]
    fn checksum_parser_rejects_hash_only_manifest_without_asset_name() {
        assert_eq!(
            expected_sha256_for_asset(
                "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                "Termy-v1.2.3-windows-x64-Setup.exe",
                false,
            ),
            None
        );
    }

    #[test]
    fn checksum_parser_rejects_ambiguous_hash_only_asset_file() {
        let checksums = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
";

        assert_eq!(
            expected_sha256_for_asset(checksums, "Termy-v1.2.3-windows-x64-Setup.exe", true),
            None
        );
    }

    #[test]
    fn team_identifier_is_parsed_from_codesign_details() {
        let details = "Executable=/Applications/Termy.app/Contents/MacOS/termy
Identifier=com.lassevestergaard.termy
Authority=Developer ID Application: Example (AB12CD34EF)
TeamIdentifier=AB12CD34EF
";
        assert_eq!(
            team_identifier_from_codesign_output(details),
            Some("AB12CD34EF".to_string())
        );
        assert_eq!(
            team_identifier_from_codesign_output("Signature=adhoc\nTeamIdentifier=not set\n"),
            None
        );
        assert_eq!(team_identifier_from_codesign_output(""), None);
    }

    #[test]
    fn app_bundle_is_resolved_from_the_running_executable() {
        assert_eq!(
            app_bundle_for_executable(Path::new(
                "/Users/me/Applications/Termy.app/Contents/MacOS/termy"
            )),
            Some(PathBuf::from("/Users/me/Applications/Termy.app"))
        );
        assert_eq!(
            app_bundle_for_executable(Path::new("/home/me/termy/target/debug/termy")),
            None
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_exe_installer_uses_silent_handoff_flags() {
        assert_eq!(
            windows_exe_installer_parameters(),
            "/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_msi_installer_quotes_installer_path() {
        let params = windows_msi_installer_parameters(Path::new(
            "C:\\Users\\me\\Downloads\\Termy Setup.msi",
        ));
        assert_eq!(
            params,
            "/i \"C:\\Users\\me\\Downloads\\Termy Setup.msi\" /passive /norestart"
        );
    }
}
