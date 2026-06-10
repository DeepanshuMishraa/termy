use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const USAGE: &str = "usage: cargo macos <run|verify|debug|logs|telemetry>";
const APP_NAME: &str = "Termy";
const EXECUTABLE_NAME: &str = "Termy";
const PRODUCT_NAME: &str = "TermySwift";
const BUNDLE_ID: &str = "com.lassevestergaard.termy";
const MIN_SYSTEM_VERSION: &str = "14.0";
const ICON_NAME: &str = "TermyIcon";
const LOGO_SOURCES: &[&str] = &["ToykoTermy", "termy_old_icon", "TermyIcon"];

#[derive(Clone, Copy)]
enum MacosCommand {
    Run,
    Verify,
    Debug,
    Logs,
    Telemetry,
}

impl MacosCommand {
    fn parse(arg: Option<String>) -> Result<Self> {
        match arg.as_deref().unwrap_or("run") {
            "run" => Ok(Self::Run),
            "verify" | "--verify" => Ok(Self::Verify),
            "debug" | "--debug" => Ok(Self::Debug),
            "logs" | "--logs" => Ok(Self::Logs),
            "telemetry" | "--telemetry" => Ok(Self::Telemetry),
            "--help" | "-h" => bail!("{USAGE}"),
            other => bail!("unknown macos command `{other}`; {USAGE}"),
        }
    }
}

struct MacosPaths {
    root_dir: PathBuf,
    macos_dir: PathBuf,
    dist_dir: PathBuf,
    app_bundle: PathBuf,
    app_macos: PathBuf,
    app_frameworks: PathBuf,
    app_resources: PathBuf,
    app_binary: PathBuf,
    info_plist: PathBuf,
    icon_source: PathBuf,
}

impl MacosPaths {
    fn new() -> Result<Self> {
        let root_dir = crate::workspace_root()?;
        let macos_dir = root_dir.join("macos");
        let dist_dir = macos_dir.join("dist");
        let app_bundle = dist_dir.join(format!("{APP_NAME}.app"));
        let app_contents = app_bundle.join("Contents");
        let app_macos = app_contents.join("MacOS");
        let app_frameworks = app_contents.join("Frameworks");
        let app_resources = app_contents.join("Resources");
        let app_binary = app_macos.join(EXECUTABLE_NAME);
        let info_plist = app_contents.join("Info.plist");
        let icon_source = root_dir.join("assets/ToykoTermy.png");

        Ok(Self {
            root_dir,
            macos_dir,
            dist_dir,
            app_bundle,
            app_macos,
            app_frameworks,
            app_resources,
            app_binary,
            info_plist,
            icon_source,
        })
    }
}

pub fn run(args: impl Iterator<Item = String>) -> Result<()> {
    require_macos()?;

    let mut args = args.filter(|arg| arg != "--");
    let command = MacosCommand::parse(args.next())?;
    if let Some(extra) = args.next() {
        bail!("unexpected macos argument `{extra}`; {USAGE}");
    }

    let paths = MacosPaths::new()?;
    stage_app(&paths)?;

    match command {
        MacosCommand::Run => open_app(&paths),
        MacosCommand::Verify => verify_app(&paths),
        MacosCommand::Debug => run_status(Command::new("lldb").arg("--").arg(&paths.app_binary)),
        MacosCommand::Logs => {
            open_app(&paths)?;
            run_status(Command::new("/usr/bin/log").args([
                "stream",
                "--info",
                "--style",
                "compact",
                "--predicate",
                &format!("process == \"{EXECUTABLE_NAME}\""),
            ]))
        }
        MacosCommand::Telemetry => {
            open_app(&paths)?;
            run_status(Command::new("/usr/bin/log").args([
                "stream",
                "--info",
                "--style",
                "compact",
                "--predicate",
                &format!("subsystem == \"{BUNDLE_ID}\""),
            ]))
        }
    }
}

fn require_macos() -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        bail!("`cargo macos` is only supported on macOS")
    }
}

fn stage_app(paths: &MacosPaths) -> Result<()> {
    kill_existing_app()?;
    build_ffi(paths)?;
    build_swift(paths)?;

    if paths.app_bundle.exists() {
        fs::remove_dir_all(&paths.app_bundle)
            .with_context(|| format!("failed to remove {}", paths.app_bundle.display()))?;
    }
    fs::create_dir_all(&paths.app_macos)
        .with_context(|| format!("failed to create {}", paths.app_macos.display()))?;
    fs::create_dir_all(&paths.app_frameworks)
        .with_context(|| format!("failed to create {}", paths.app_frameworks.display()))?;
    fs::create_dir_all(&paths.app_resources)
        .with_context(|| format!("failed to create {}", paths.app_resources.display()))?;

    let swift_binary = swift_build_binary(paths)?;
    fs::copy(&swift_binary, &paths.app_binary).with_context(|| {
        format!(
            "failed to copy {} to {}",
            swift_binary.display(),
            paths.app_binary.display()
        )
    })?;
    make_executable(&paths.app_binary)?;
    bundle_ffi_dylib(paths)?;
    bundle_cli(paths)?;

    build_icon(paths)?;
    copy_logo_assets(paths)?;
    write_info_plist(paths)?;
    ad_hoc_sign(paths)?;
    Ok(())
}

fn kill_existing_app() -> Result<()> {
    run_ignore_failure(Command::new("pkill").args(["-x", EXECUTABLE_NAME]))?;
    run_ignore_failure(Command::new("pkill").args(["-x", APP_NAME]))?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    run_ignore_failure(Command::new("pkill").args(["-9", "-x", EXECUTABLE_NAME]))?;
    run_ignore_failure(Command::new("pkill").args(["-9", "-x", APP_NAME]))
}

fn build_ffi(paths: &MacosPaths) -> Result<()> {
    // Builds the FFI dylib plus the `termy-cli` binary that gets bundled for the
    // in-app "Install Command Line Tool" action.
    run_status(
        Command::new("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(paths.root_dir.join("Cargo.toml"))
            .args(["-p", "termy_ffi", "-p", "termy_cli"]),
    )
}

fn build_swift(paths: &MacosPaths) -> Result<()> {
    run_status(
        Command::new("swift")
            .arg("build")
            .arg("--package-path")
            .arg(&paths.macos_dir),
    )
}

fn bundle_ffi_dylib(paths: &MacosPaths) -> Result<()> {
    let source = ffi_dylib_path(paths)?;
    let bundled = paths.app_frameworks.join("libtermy_ffi.dylib");
    fs::copy(&source, &bundled).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            bundled.display()
        )
    })?;

    run_status_captured(
        Command::new("install_name_tool")
            .arg("-id")
            .arg("@rpath/libtermy_ffi.dylib")
            .arg(&bundled),
    )?;

    let linked_path = linked_ffi_path(&paths.app_binary)?;
    run_status_captured(
        Command::new("install_name_tool")
            .arg("-change")
            .arg(&linked_path)
            .arg("@rpath/libtermy_ffi.dylib")
            .arg(&paths.app_binary),
    )
}

fn bundle_cli(paths: &MacosPaths) -> Result<()> {
    let source = paths.root_dir.join("target/debug/termy-cli");
    if !source.exists() {
        bail!("could not find built termy-cli under target/debug");
    }
    let bundled = paths.app_macos.join("termy-cli");
    fs::copy(&source, &bundled).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            bundled.display()
        )
    })?;
    make_executable(&bundled)
}

fn ffi_dylib_path(paths: &MacosPaths) -> Result<PathBuf> {
    for candidate in [
        paths.root_dir.join("target/debug/libtermy_ffi.dylib"),
        paths.root_dir.join("target/debug/deps/libtermy_ffi.dylib"),
    ] {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("could not find built libtermy_ffi.dylib under target/debug")
}

fn linked_ffi_path(app_binary: &Path) -> Result<String> {
    let output = Command::new("otool")
        .arg("-L")
        .arg(app_binary)
        .output()
        .with_context(|| format!("failed to inspect {}", app_binary.display()))?;
    if !output.status.success() {
        bail!(
            "`otool -L {}` failed: {}",
            app_binary.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let output = String::from_utf8(output.stdout).context("otool output was not utf-8")?;
    output
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .find(|path| path.contains("libtermy_ffi.dylib"))
        .map(str::to_owned)
        .with_context(|| {
            format!(
                "{} is not linked against libtermy_ffi.dylib",
                app_binary.display()
            )
        })
}

fn ad_hoc_sign(paths: &MacosPaths) -> Result<()> {
    for path in [
        paths.app_frameworks.join("libtermy_ffi.dylib"),
        paths.app_binary.clone(),
        paths.app_bundle.clone(),
    ] {
        run_status_captured(
            Command::new("codesign")
                .arg("--force")
                .arg("--sign")
                .arg("-")
                .arg("--timestamp=none")
                .arg(path),
        )?;
    }
    Ok(())
}

fn swift_build_binary(paths: &MacosPaths) -> Result<PathBuf> {
    let output = Command::new("swift")
        .arg("build")
        .arg("--package-path")
        .arg(&paths.macos_dir)
        .arg("--show-bin-path")
        .output()
        .context("failed to run `swift build --show-bin-path`")?;
    if !output.status.success() {
        bail!(
            "`swift build --show-bin-path` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let bin_dir = String::from_utf8(output.stdout)
        .context("swift build output was not utf-8")?
        .trim()
        .to_string();
    Ok(PathBuf::from(bin_dir).join(PRODUCT_NAME))
}

fn build_icon(paths: &MacosPaths) -> Result<()> {
    let icon_tmp = tempfile::Builder::new()
        .prefix(&format!("{ICON_NAME}."))
        .tempdir_in(&paths.dist_dir)
        .with_context(|| format!("failed to create temp dir in {}", paths.dist_dir.display()))?;
    let iconset = icon_tmp.path().join(format!("{ICON_NAME}.iconset"));
    fs::create_dir_all(&iconset)
        .with_context(|| format!("failed to create {}", iconset.display()))?;

    for (size, scale, filename) in [
        (16, 1, "icon_16x16.png"),
        (16, 2, "icon_16x16@2x.png"),
        (32, 1, "icon_32x32.png"),
        (32, 2, "icon_32x32@2x.png"),
        (128, 1, "icon_128x128.png"),
        (128, 2, "icon_128x128@2x.png"),
        (256, 1, "icon_256x256.png"),
        (256, 2, "icon_256x256@2x.png"),
        (512, 1, "icon_512x512.png"),
        (512, 2, "icon_512x512@2x.png"),
    ] {
        let pixels = size * scale;
        run_status_quiet(
            Command::new("sips")
                .args(["-z", &pixels.to_string(), &pixels.to_string()])
                .arg(&paths.icon_source)
                .arg("--out")
                .arg(iconset.join(filename)),
        )?;
    }

    run_status(
        Command::new("iconutil")
            .args(["-c", "icns"])
            .arg(&iconset)
            .arg("-o")
            .arg(paths.app_resources.join(format!("{ICON_NAME}.icns"))),
    )
}

fn copy_logo_assets(paths: &MacosPaths) -> Result<()> {
    for logo in LOGO_SOURCES {
        let source = paths.root_dir.join(format!("assets/{logo}.png"));
        if source.exists() {
            fs::copy(&source, paths.app_resources.join(format!("{logo}.png")))
                .with_context(|| format!("failed to copy logo asset {}", source.display()))?;
        } else {
            eprintln!("warning: logo asset not found: assets/{logo}.png");
        }
    }
    Ok(())
}

fn write_info_plist(paths: &MacosPaths) -> Result<()> {
    fs::write(
        &paths.info_plist,
        info_plist(&app_version(&paths.root_dir)?),
    )
    .with_context(|| format!("failed to write {}", paths.info_plist.display()))
}

fn app_version(root_dir: &Path) -> Result<String> {
    let cargo_toml = fs::read_to_string(root_dir.join("crates/desktop_app/Cargo.toml"))
        .context("failed to read crates/desktop_app/Cargo.toml")?;
    Ok(cargo_toml
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = \"")
                .and_then(|value| value.strip_suffix('"'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "0.0.0".to_string()))
}

fn info_plist(app_version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>{EXECUTABLE_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>{BUNDLE_ID}</string>
  <key>CFBundleDisplayName</key>
  <string>{APP_NAME}</string>
  <key>CFBundleIconFile</key>
  <string>{ICON_NAME}</string>
  <key>CFBundleName</key>
  <string>{APP_NAME}</string>
  <key>CFBundleShortVersionString</key>
  <string>{app_version}</string>
  <key>CFBundleVersion</key>
  <string>{app_version}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>LSMinimumSystemVersion</key>
  <string>{MIN_SYSTEM_VERSION}</string>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key>
      <string>{BUNDLE_ID}</string>
      <key>CFBundleURLSchemes</key>
      <array>
        <string>termy</string>
      </array>
    </dict>
  </array>
</dict>
</plist>
"#
    )
}

fn open_app(paths: &MacosPaths) -> Result<()> {
    run_status(
        Command::new("/usr/bin/open")
            .arg("-n")
            .arg(&paths.app_bundle),
    )
}

fn verify_app(paths: &MacosPaths) -> Result<()> {
    open_app(paths)?;
    std::thread::sleep(std::time::Duration::from_secs(1));
    run_status(Command::new("pgrep").arg("-f").arg(&paths.app_binary))
}

fn run_status(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {:?}", command))?;
    if !status.success() {
        bail!("command failed with {status}: {:?}", command);
    }
    Ok(())
}

fn run_ignore_failure(command: &mut Command) -> Result<()> {
    command
        .status()
        .with_context(|| format!("failed to run {:?}", command))?;
    Ok(())
}

fn run_status_quiet(command: &mut Command) -> Result<()> {
    command.stdout(Stdio::null());
    run_status(command)
}

fn run_status_captured(command: &mut Command) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("failed to run {:?}", command))?;
    if !output.status.success() {
        bail!(
            "command failed with {}: {:?}\n{}",
            output.status,
            command,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to chmod {}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}
