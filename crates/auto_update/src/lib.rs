mod github;

pub use github::{ReleaseInfo, fetch_latest_release};

use anyhow::{Context, Result};
use gpui::{App, AsyncApp, WeakEntity};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        url: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: u64,
    },
    Downloaded {
        version: String,
        dmg_path: PathBuf,
    },
    Installing {
        version: String,
    },
    Installed {
        version: String,
    },
    Error(String),
}

pub struct AutoUpdater {
    current_version: &'static str,
    pub state: UpdateState,
}

impl AutoUpdater {
    pub fn new(current_version: &'static str) -> Self {
        Self {
            current_version,
            state: UpdateState::Idle,
        }
    }

    pub fn check(entity: WeakEntity<Self>, cx: &mut App) {
        let Some(this) = entity.upgrade() else { return };
        this.update(cx, |this, cx| {
            this.state = UpdateState::Checking;
            cx.notify();
        });

        let current_version = this.read(cx).current_version.to_string();
        let bg = cx
            .background_executor()
            .spawn(async move { fetch_latest_release() });

        let weak = entity.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = bg.await;
            let _ = cx.update(|cx| {
                let Some(this) = weak.upgrade() else { return };
                this.update(cx, |this, cx| {
                    match result {
                        Ok(info) => {
                            let current = semver::Version::parse(&current_version).ok();
                            let latest = semver::Version::parse(&info.version).ok();
                            match (current, latest) {
                                (Some(c), Some(l)) if l > c => {
                                    this.state = UpdateState::Available {
                                        version: info.version,
                                        url: info.download_url,
                                    };
                                }
                                _ => {
                                    this.state = UpdateState::UpToDate;
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Update check failed: {}", e);
                            this.state = UpdateState::Error(format!("{}", e));
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub fn install(entity: WeakEntity<Self>, cx: &mut App) {
        let Some(this) = entity.upgrade() else { return };

        let (version, url) = {
            let read = this.read(cx);
            match &read.state {
                UpdateState::Available { version, url } => (version.clone(), url.clone()),
                _ => return,
            }
        };

        this.update(cx, |this, cx| {
            this.state = UpdateState::Downloading {
                version: version.clone(),
                downloaded: 0,
                total: 0,
            };
            cx.notify();
        });

        let (progress_tx, progress_rx) = flume::bounded::<(u64, u64)>(4);
        let dest = cache_dmg_path(&version);
        let dl_version = version.clone();
        let bg = cx
            .background_executor()
            .spawn(async move { download_dmg(&url, &dest, progress_tx) });

        // Progress reader
        let weak_progress = entity.clone();
        let progress_version = version.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok((downloaded, total)) = progress_rx.recv_async().await {
                let Some(this) = weak_progress.upgrade() else {
                    break;
                };
                let ver = progress_version.clone();
                cx.update(|cx| {
                    this.update(cx, |this, cx| {
                        this.state = UpdateState::Downloading {
                            version: ver,
                            downloaded,
                            total,
                        };
                        cx.notify();
                    });
                });
            }
        })
        .detach();

        // Download completion
        let weak_done = entity.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = bg.await;
            let _ = cx.update(|cx| {
                let Some(this) = weak_done.upgrade() else {
                    return;
                };
                this.update(cx, |this, cx| {
                    match result {
                        Ok(path) => {
                            this.state = UpdateState::Downloaded {
                                version: dl_version,
                                dmg_path: path,
                            };
                        }
                        Err(e) => {
                            this.state = UpdateState::Error(format!("Download failed: {}", e));
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub fn complete_install(entity: WeakEntity<Self>, cx: &mut App) {
        let Some(this) = entity.upgrade() else { return };

        let (version, dmg_path) = {
            let read = this.read(cx);
            match &read.state {
                UpdateState::Downloaded { version, dmg_path } => {
                    (version.clone(), dmg_path.clone())
                }
                _ => return,
            }
        };

        this.update(cx, |this, cx| {
            this.state = UpdateState::Installing {
                version: version.clone(),
            };
            cx.notify();
        });

        let bg = cx
            .background_executor()
            .spawn(async move { do_install(&dmg_path) });

        let weak = entity.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = bg.await;
            let _ = cx.update(|cx| {
                let Some(this) = weak.upgrade() else { return };
                this.update(cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            this.state = UpdateState::Installed { version };
                        }
                        Err(e) => {
                            this.state = UpdateState::Error(format!("Install failed: {}", e));
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub fn dismiss(&mut self, cx: &mut gpui::Context<Self>) {
        self.state = UpdateState::Idle;
        cx.notify();
    }
}

fn cache_dmg_path(version: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let cache_dir = PathBuf::from(home).join("Library/Caches/Termy");
    let _ = std::fs::create_dir_all(&cache_dir);
    cache_dir.join(format!("update-{}.dmg", version))
}

fn download_dmg(
    url: &str,
    dest: &PathBuf,
    progress_tx: flume::Sender<(u64, u64)>,
) -> Result<PathBuf> {
    let response = ureq::get(url)
        .set("User-Agent", "Termy-Updater/1.0")
        .call()
        .context("Failed to download DMG")?;

    let total: u64 = response
        .header("Content-Length")
        .and_then(|h| h.parse().ok())
        .unwrap_or(0);

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(dest).context("Failed to create DMG file")?;
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

    Ok(dest.clone())
}

#[cfg(target_os = "macos")]
fn do_install(dmg_path: &PathBuf) -> Result<()> {
    use std::process::Command;

    // Mount the DMG
    let mount = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-quiet"])
        .arg(dmg_path)
        .output()
        .context("Failed to mount DMG")?;

    if !mount.status.success() {
        anyhow::bail!(
            "hdiutil attach failed: {}",
            String::from_utf8_lossy(&mount.stderr)
        );
    }

    // Copy the app bundle
    let cp_result = Command::new("cp")
        .args(["-rf", "/Volumes/Termy/Termy.app", "/Applications/Termy.app"])
        .output()
        .context("Failed to copy Termy.app")?;

    // Always try to detach, even if copy failed
    let _ = Command::new("hdiutil")
        .args(["detach", "/Volumes/Termy", "-quiet"])
        .output();

    if !cp_result.status.success() {
        anyhow::bail!("cp failed: {}", String::from_utf8_lossy(&cp_result.stderr));
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn do_install(_dmg_path: &PathBuf) -> Result<()> {
    anyhow::bail!("Auto-install is only supported on macOS")
}
