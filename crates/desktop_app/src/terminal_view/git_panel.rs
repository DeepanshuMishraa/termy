//! Native git panel docked to the right of the terminal, in the spirit of
//! Zed's git panel: changed files with stage checkboxes and +/− counts, a
//! commit message box, and a commit button.
//!
//! All git work shells out to the `git` CLI on background threads; results
//! are published into [`GitPanelShared`] and picked up on the next render
//! (the PTY wakeup channel nudges a redraw). The panel polls while open so
//! external changes (edits, commits from the terminal) show up without a
//! manual refresh.

use super::*;
use gpui::prelude::FluentBuilder as _;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

pub(super) const GIT_PANEL_WIDTH: f32 = 320.0;
const GIT_PANEL_POLL_INTERVAL: Duration = Duration::from_millis(3000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GitFileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl GitFileStatus {
    pub(super) fn glyph(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Untracked => "U",
            Self::Conflicted => "!",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GitStageState {
    Staged,
    Unstaged,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GitFileEntry {
    /// Repo-relative path.
    pub(super) path: String,
    pub(super) status: GitFileStatus,
    pub(super) stage: GitStageState,
    pub(super) additions: Option<u64>,
    pub(super) deletions: Option<u64>,
}

impl GitFileEntry {
    pub(super) fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(self.path.as_str())
    }

    pub(super) fn dir_hint(&self) -> Option<&str> {
        self.path.rsplit_once('/').map(|(dir, _)| dir)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(super) struct GitStatusSnapshot {
    /// `None` when the working directory is not inside a git repository.
    pub(super) repo_root: Option<PathBuf>,
    pub(super) branch: String,
    pub(super) entries: Vec<GitFileEntry>,
    pub(super) error: Option<String>,
}

impl GitStatusSnapshot {
    pub(super) fn total_additions(&self) -> u64 {
        self.entries
            .iter()
            .filter_map(|entry| entry.additions)
            .sum()
    }

    pub(super) fn total_deletions(&self) -> u64 {
        self.entries
            .iter()
            .filter_map(|entry| entry.deletions)
            .sum()
    }

    pub(super) fn staged_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.stage, GitStageState::Staged | GitStageState::Partial))
            .count()
    }
}

/// State shared with the background git threads.
pub(super) struct GitPanelShared {
    snapshot: Mutex<Option<GitStatusSnapshot>>,
    generation: AtomicU64,
    busy: AtomicBool,
    /// Set by a successful commit so the view clears the message input.
    clear_commit_input: AtomicBool,
}

impl GitPanelShared {
    fn publish(&self, snapshot: GitStatusSnapshot) {
        if let Ok(mut slot) = self.snapshot.lock() {
            *slot = Some(snapshot);
        }
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.busy.store(false, std::sync::atomic::Ordering::Release);
    }
}

pub(super) struct GitPanelState {
    shared: Arc<GitPanelShared>,
    pub(super) snapshot: GitStatusSnapshot,
    applied_generation: u64,
    pub(super) commit_input: InlineInputState,
    pub(super) editing_commit: bool,
    pub(super) list_scroll: gpui::ScrollHandle,
    poll_scheduled: bool,
}

impl GitPanelState {
    pub(super) fn new() -> Self {
        Self {
            shared: Arc::new(GitPanelShared {
                snapshot: Mutex::new(None),
                generation: AtomicU64::new(0),
                busy: AtomicBool::new(false),
                clear_commit_input: AtomicBool::new(false),
            }),
            snapshot: GitStatusSnapshot::default(),
            applied_generation: 0,
            commit_input: InlineInputState::new(String::new()),
            editing_commit: false,
            list_scroll: gpui::ScrollHandle::new(),
            poll_scheduled: false,
        }
    }
}

fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr
            .trim()
            .lines()
            .next()
            .unwrap_or("git failed")
            .to_string())
    }
}

/// Parse `git status --porcelain=v1 -z` output into (path, status, stage).
fn parse_porcelain_status(output: &str) -> Vec<GitFileEntry> {
    let mut entries = Vec::new();
    let mut fields = output.split('\0');
    while let Some(record) = fields.next() {
        if record.len() < 4 {
            continue;
        }
        let index_code = record.as_bytes()[0] as char;
        let worktree_code = record.as_bytes()[1] as char;
        let path = record[3..].to_string();
        // Rename/copy records carry the original path as the next field.
        if matches!(index_code, 'R' | 'C') || matches!(worktree_code, 'R' | 'C') {
            let _original = fields.next();
        }

        let status = if index_code == '?' || worktree_code == '?' {
            GitFileStatus::Untracked
        } else if index_code == 'U'
            || worktree_code == 'U'
            || (index_code == 'A' && worktree_code == 'A')
            || (index_code == 'D' && worktree_code == 'D')
        {
            GitFileStatus::Conflicted
        } else {
            let primary = if index_code != ' ' {
                index_code
            } else {
                worktree_code
            };
            match primary {
                'A' => GitFileStatus::Added,
                'D' => GitFileStatus::Deleted,
                'R' | 'C' => GitFileStatus::Renamed,
                _ => GitFileStatus::Modified,
            }
        };

        let stage = if status == GitFileStatus::Untracked {
            GitStageState::Unstaged
        } else {
            let staged = index_code != ' ' && index_code != '?';
            let unstaged = worktree_code != ' ' && worktree_code != '?';
            match (staged, unstaged) {
                (true, true) => GitStageState::Partial,
                (true, false) => GitStageState::Staged,
                _ => GitStageState::Unstaged,
            }
        };

        entries.push(GitFileEntry {
            path,
            status,
            stage,
            additions: None,
            deletions: None,
        });
    }
    entries.sort_by(|left, right| {
        let left_untracked = left.status == GitFileStatus::Untracked;
        let right_untracked = right.status == GitFileStatus::Untracked;
        left_untracked
            .cmp(&right_untracked)
            .then_with(|| left.path.cmp(&right.path))
    });
    entries
}

/// Merge `git diff --numstat` lines ("added<TAB>deleted<TAB>path") into a
/// per-path map; binary files ("-") contribute no counts.
fn merge_numstat(map: &mut HashMap<String, (u64, u64)>, output: &str) {
    for line in output.lines() {
        let mut parts = line.split('\t');
        let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (Ok(added), Ok(deleted)) = (added.parse::<u64>(), deleted.parse::<u64>()) else {
            continue;
        };
        // Rename lines render as "old => new" or "{a => b}/rest"; index by
        // the new name to match porcelain output.
        let path = normalize_numstat_path(path);
        let entry = map.entry(path).or_insert((0, 0));
        entry.0 += added;
        entry.1 += deleted;
    }
}

fn normalize_numstat_path(path: &str) -> String {
    if let (Some(open), Some(close)) = (path.find('{'), path.find('}'))
        && open < close
    {
        let segment = &path[open + 1..close];
        let renamed = segment.rsplit(" => ").next().unwrap_or(segment);
        let mut normalized = format!("{}{}{}", &path[..open], renamed, &path[close + 1..]);
        if let Some(stripped) = normalized.strip_prefix('/') {
            normalized = stripped.to_string();
        }
        return normalized.replace("//", "/");
    }
    path.rsplit(" => ").next().unwrap_or(path).to_string()
}

/// Gather the full panel snapshot for the repository containing `cwd`.
fn collect_git_status(cwd: &Path) -> GitStatusSnapshot {
    let repo_root = match run_git(cwd, &["rev-parse", "--show-toplevel"]) {
        Ok(root) => PathBuf::from(root.trim()),
        Err(_) => {
            return GitStatusSnapshot::default();
        }
    };

    let branch = run_git(&repo_root, &["rev-parse", "--abbrev-ref", "HEAD"]).map_or_else(
        |_| "(no commits)".to_string(),
        |branch| branch.trim().to_string(),
    );

    let mut snapshot = GitStatusSnapshot {
        repo_root: Some(repo_root.clone()),
        branch,
        entries: Vec::new(),
        error: None,
    };

    match run_git(&repo_root, &["status", "--porcelain=v1", "-z"]) {
        Ok(output) => snapshot.entries = parse_porcelain_status(&output),
        Err(error) => {
            snapshot.error = Some(error);
            return snapshot;
        }
    }

    let mut counts: HashMap<String, (u64, u64)> = HashMap::new();
    if let Ok(output) = run_git(&repo_root, &["diff", "--numstat"]) {
        merge_numstat(&mut counts, &output);
    }
    if let Ok(output) = run_git(&repo_root, &["diff", "--numstat", "--cached"]) {
        merge_numstat(&mut counts, &output);
    }
    for entry in &mut snapshot.entries {
        if let Some((added, deleted)) = counts.get(&entry.path).copied() {
            entry.additions = Some(added);
            entry.deletions = Some(deleted);
        }
    }

    snapshot
}

enum GitPanelAction {
    Stage(String),
    Unstage(String),
    StageAll,
    UnstageAll,
    Commit {
        message: String,
        include_tracked: bool,
    },
}

impl TerminalView {
    pub(super) fn git_panel_visible(&self) -> bool {
        self.git_panel_enabled && self.git_panel_open && !self.simple_mode
    }

    /// Width reserved on the right for the git panel; feeds the terminal
    /// grid sizer and content bounds.
    pub(super) fn git_panel_width_px(&self) -> f32 {
        if self.git_panel_visible() {
            GIT_PANEL_WIDTH
        } else {
            0.0
        }
    }

    pub(super) fn git_commit_editing(&self) -> bool {
        self.git_panel_visible() && self.git_panel.editing_commit
    }

    pub(crate) fn toggle_git_panel(&mut self, cx: &mut Context<Self>) {
        if !self.git_panel_enabled {
            termy_toast::info("Enable Git Panel in Settings to use this command");
            self.notify_overlay(cx);
            return;
        }
        self.git_panel_open = !self.git_panel_open;
        if self.git_panel_open {
            self.refresh_git_panel();
            self.schedule_git_panel_poll(cx);
        } else {
            self.git_panel.editing_commit = false;
        }
        self.mark_tab_strip_layout_dirty();
        cx.notify();
    }

    /// Working directory the panel inspects: the active tab's prompt cwd
    /// when shell integration reported one, otherwise the configured launch
    /// directory.
    fn git_panel_working_dir(&self) -> Option<PathBuf> {
        let from_tab = self
            .active_tab_ref()
            .and_then(|tab| tab.last_prompt_cwd.clone());
        from_tab
            .map(PathBuf::from)
            .or_else(|| {
                self.persisted_native_workspace_working_dir()
                    .map(PathBuf::from)
            })
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    }

    pub(super) fn refresh_git_panel(&mut self) {
        if !self.git_panel_visible() {
            return;
        }
        let shared = self.git_panel.shared.clone();
        if shared.busy.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return;
        }
        let Some(cwd) = self.git_panel_working_dir() else {
            shared
                .busy
                .store(false, std::sync::atomic::Ordering::Release);
            return;
        };
        let wakeup = self.event_wakeup_tx.clone();
        std::thread::spawn(move || {
            let snapshot = collect_git_status(&cwd);
            shared.publish(snapshot);
            let _ = wakeup.try_send(());
        });
    }

    fn run_git_panel_action(&mut self, action: GitPanelAction) {
        let Some(repo_root) = self.git_panel.snapshot.repo_root.clone() else {
            return;
        };
        let shared = self.git_panel.shared.clone();
        let wakeup = self.event_wakeup_tx.clone();
        std::thread::spawn(move || {
            let result = match &action {
                GitPanelAction::Stage(path) => run_git(&repo_root, &["add", "--", path]),
                GitPanelAction::Unstage(path) => {
                    run_git(&repo_root, &["reset", "-q", "HEAD", "--", path])
                }
                GitPanelAction::StageAll => run_git(&repo_root, &["add", "-A"]),
                GitPanelAction::UnstageAll => run_git(&repo_root, &["reset", "-q", "HEAD", "--"]),
                GitPanelAction::Commit {
                    message,
                    include_tracked,
                } => {
                    let result = if *include_tracked {
                        run_git(&repo_root, &["commit", "-a", "-m", message])
                    } else {
                        run_git(&repo_root, &["commit", "-m", message])
                    };
                    if result.is_ok() {
                        shared
                            .clear_commit_input
                            .store(true, std::sync::atomic::Ordering::Release);
                        termy_toast::success("Committed");
                    }
                    result
                }
            };
            if let Err(error) = result {
                termy_toast::error(format!("git: {error}"));
            }
            let snapshot = collect_git_status(&repo_root);
            shared.publish(snapshot);
            let _ = wakeup.try_send(());
        });
    }

    pub(super) fn git_panel_toggle_stage(&mut self, path: String, stage: GitStageState) {
        let action = match stage {
            GitStageState::Staged => GitPanelAction::Unstage(path),
            GitStageState::Unstaged | GitStageState::Partial => GitPanelAction::Stage(path),
        };
        self.run_git_panel_action(action);
    }

    pub(super) fn git_panel_stage_all(&mut self) {
        self.run_git_panel_action(GitPanelAction::StageAll);
    }

    pub(super) fn git_panel_unstage_all(&mut self) {
        self.run_git_panel_action(GitPanelAction::UnstageAll);
    }

    pub(super) fn git_panel_commit(&mut self, cx: &mut Context<Self>) {
        let message = self.git_panel.commit_input.text().trim().to_string();
        if message.is_empty() {
            termy_toast::info("Enter a commit message first");
            self.notify_overlay(cx);
            return;
        }
        let include_tracked = self.git_panel.snapshot.staged_count() == 0;
        self.git_panel.editing_commit = false;
        self.run_git_panel_action(GitPanelAction::Commit {
            message,
            include_tracked,
        });
        cx.notify();
    }

    pub(super) fn begin_git_commit_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.git_panel_visible() {
            return;
        }
        self.git_panel.editing_commit = true;
        self.focus_handle.focus(window, cx);
        self.reset_cursor_blink_phase();
        self.inline_input_selecting = false;
        cx.notify();
    }

    pub(super) fn cancel_git_commit_edit(&mut self, cx: &mut Context<Self>) {
        if self.git_panel.editing_commit {
            self.git_panel.editing_commit = false;
            cx.notify();
        }
    }

    /// Pick up snapshots published by background threads. Called per render.
    pub(super) fn apply_git_panel_updates(&mut self) {
        let generation = self
            .git_panel
            .shared
            .generation
            .load(std::sync::atomic::Ordering::Acquire);
        if generation != self.git_panel.applied_generation {
            self.git_panel.applied_generation = generation;
            if let Ok(slot) = self.git_panel.shared.snapshot.lock()
                && let Some(snapshot) = slot.clone()
            {
                self.git_panel.snapshot = snapshot;
            }
        }
        if self
            .git_panel
            .shared
            .clear_commit_input
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.git_panel.commit_input.set_text(String::new());
        }
    }

    /// Keep the snapshot fresh while the panel is open: external edits and
    /// commits made in the terminal show up within one poll interval.
    pub(super) fn schedule_git_panel_poll(&mut self, cx: &mut Context<Self>) {
        if self.git_panel.poll_scheduled || !self.git_panel_visible() {
            return;
        }
        self.git_panel.poll_scheduled = true;
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            smol::Timer::after(GIT_PANEL_POLL_INTERVAL).await;
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.git_panel.poll_scheduled = false;
                    if view.git_panel_visible() {
                        view.refresh_git_panel();
                        view.schedule_git_panel_poll(cx);
                    }
                })
            });
        })
        .detach();
    }
}

impl TerminalView {
    fn git_status_color(&self, status: GitFileStatus) -> gpui::Rgba {
        match status {
            GitFileStatus::Modified | GitFileStatus::Renamed => self.colors.ansi[3],
            GitFileStatus::Added | GitFileStatus::Untracked => self.colors.ansi[2],
            GitFileStatus::Deleted | GitFileStatus::Conflicted => self.colors.ansi[1],
        }
    }

    /// The right-docked git panel: header, changed-file rows with stage
    /// checkboxes, and the commit box.
    pub(super) fn render_git_panel(
        &mut self,
        colors: &TerminalColors,
        ui_font_family: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut stroke = colors.foreground;
        stroke.a = self.scaled_chrome_alpha(0.12);
        let mut text_primary = colors.foreground;
        text_primary.a = 0.92;
        let mut text_muted = colors.foreground;
        text_muted.a = 0.55;
        let mut hover_bg = colors.foreground;
        hover_bg.a = self.scaled_chrome_alpha(0.06);
        let mut field_bg = colors.foreground;
        field_bg.a = self.scaled_chrome_alpha(0.06);
        let mut accent = colors.cursor;
        accent.a = self.scaled_chrome_accent_alpha(0.85);
        let mut additions_color = self.colors.ansi[2];
        additions_color.a = 0.9;
        let mut deletions_color = self.colors.ansi[1];
        deletions_color.a = 0.9;
        let mut selection: gpui::Rgba = colors.cursor;
        selection.a = 0.30;

        let snapshot = self.git_panel.snapshot.clone();
        let is_repo = snapshot.repo_root.is_some();
        let staged_count = snapshot.staged_count();
        let editing_commit = self.git_panel.editing_commit;
        let commit_message_empty = self.git_panel.commit_input.text().trim().is_empty();

        // Header: "Changes (N)" + branch.
        let header = div()
            .id("git-panel-header")
            .flex_none()
            .w_full()
            .h(px(34.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(stroke)
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(text_primary)
                    .font_family(ui_font_family.clone())
                    .child(format!("Changes ({})", snapshot.entries.len())),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(text_muted)
                    .font_family(ui_font_family.clone())
                    .max_w(px(140.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(snapshot.branch.clone()),
            );

        // Summary: total +/- and the stage-all / unstage-all toggle.
        let all_staged = !snapshot.entries.is_empty() && staged_count == snapshot.entries.len();
        let stage_all_label = if all_staged {
            "Unstage All"
        } else {
            "Stage All"
        };
        let summary = div()
            .id("git-panel-summary")
            .flex_none()
            .w_full()
            .h(px(28.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(stroke)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .font_family(ui_font_family.clone())
                    .text_size(px(11.0))
                    .child(
                        div()
                            .text_color(additions_color)
                            .child(format!("+{}", snapshot.total_additions())),
                    )
                    .child(
                        div()
                            .text_color(deletions_color)
                            .child(format!("\u{2212}{}", snapshot.total_deletions())),
                    ),
            )
            .when(is_repo && !snapshot.entries.is_empty(), |row| {
                row.child(
                    div()
                        .id("git-panel-stage-all")
                        .px(px(8.0))
                        .py(px(3.0))
                        .rounded(px(4.0))
                        .text_size(px(11.0))
                        .font_family(ui_font_family.clone())
                        .text_color(text_primary)
                        .hover(move |style| style.bg(hover_bg))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _event: &MouseDownEvent, _window, cx| {
                                if all_staged {
                                    view.git_panel_unstage_all();
                                } else {
                                    view.git_panel_stage_all();
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .child(stage_all_label),
                )
            });

        // File rows.
        let mut rows = div()
            .id("git-panel-rows-content")
            .flex()
            .flex_col()
            .w_full()
            .py(px(2.0));
        if !is_repo {
            rows = rows.child(
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_size(px(11.0))
                    .text_color(text_muted)
                    .font_family(ui_font_family.clone())
                    .child("Not a git repository"),
            );
        } else if let Some(error) = snapshot.error.clone() {
            rows = rows.child(
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_size(px(11.0))
                    .text_color(deletions_color)
                    .font_family(ui_font_family.clone())
                    .child(error),
            );
        } else if snapshot.entries.is_empty() {
            rows = rows.child(
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_size(px(11.0))
                    .text_color(text_muted)
                    .font_family(ui_font_family.clone())
                    .child("No changes"),
            );
        }
        for (index, entry) in snapshot.entries.iter().enumerate() {
            let status_color = self.git_status_color(entry.status);
            let staged = matches!(entry.stage, GitStageState::Staged);
            let partial = matches!(entry.stage, GitStageState::Partial);
            let toggle_path = entry.path.clone();
            let toggle_stage = entry.stage;
            let mut checkbox_border = colors.foreground;
            checkbox_border.a = 0.35;
            let mut checkbox_fill = accent;
            checkbox_fill.a = if staged || partial { accent.a } else { 0.0 };
            let mut check_glyph_color = colors.background;
            check_glyph_color.a = 0.95;

            rows = rows.child(
                div()
                    .id(("git-panel-row", index))
                    .w_full()
                    .h(px(26.0))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .hover(move |style| style.bg(hover_bg))
                    .child(
                        div()
                            .flex_none()
                            .w(px(12.0))
                            .text_size(px(11.0))
                            .text_color(status_color)
                            .font_family(ui_font_family.clone())
                            .child(entry.status.glyph()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .items_baseline()
                            .gap(px(6.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(12.0))
                                    .text_color(text_primary)
                                    .font_family(ui_font_family.clone())
                                    .child(entry.file_name().to_string()),
                            )
                            .when_some(entry.dir_hint(), |row, dir| {
                                row.child(
                                    div()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_size(px(10.0))
                                        .text_color(text_muted)
                                        .font_family(ui_font_family.clone())
                                        .child(dir.to_string()),
                                )
                            }),
                    )
                    .when_some(entry.additions, |row, additions| {
                        row.child(
                            div()
                                .flex_none()
                                .text_size(px(10.0))
                                .text_color(additions_color)
                                .font_family(ui_font_family.clone())
                                .child(format!("+{additions}")),
                        )
                    })
                    .when_some(entry.deletions, |row, deletions| {
                        row.child(
                            div()
                                .flex_none()
                                .text_size(px(10.0))
                                .text_color(deletions_color)
                                .font_family(ui_font_family.clone())
                                .child(format!("\u{2212}{deletions}")),
                        )
                    })
                    .child(
                        div()
                            .id(("git-panel-stage", index))
                            .flex_none()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded(px(3.0))
                            .border_1()
                            .border_color(checkbox_border)
                            .bg(checkbox_fill)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _event: &MouseDownEvent, _window, cx| {
                                    view.git_panel_toggle_stage(toggle_path.clone(), toggle_stage);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .when(staged, |checkbox| {
                                checkbox.child(
                                    div()
                                        .text_size(px(9.0))
                                        .text_color(check_glyph_color)
                                        .child("\u{2713}"),
                                )
                            })
                            .when(partial, |checkbox| {
                                checkbox.child(
                                    div()
                                        .text_size(px(9.0))
                                        .text_color(check_glyph_color)
                                        .child("\u{2212}"),
                                )
                            }),
                    ),
            );
        }

        // Commit box: message input + commit button.
        let commit_label = if staged_count == 0 {
            "Commit Tracked"
        } else {
            "Commit"
        };
        let commit_message: AnyElement = if editing_commit {
            div()
                .relative()
                .w_full()
                .h(px(24.0))
                .child(self.render_inline_input_layer(
                    Font {
                        family: ui_font_family.clone(),
                        ..Font::default()
                    },
                    px(12.0),
                    text_primary.into(),
                    selection.into(),
                    InlineInputAlignment::Left,
                    cx,
                ))
                .into_any_element()
        } else {
            let message = self.git_panel.commit_input.text().trim().to_string();
            let (display, display_color) = if message.is_empty() {
                ("Enter commit message".to_string(), text_muted)
            } else {
                (message, text_primary)
            };
            div()
                .id("git-panel-commit-message")
                .w_full()
                .h(px(24.0))
                .px(px(6.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(12.0))
                .text_color(display_color)
                .font_family(ui_font_family.clone())
                .cursor_text()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _event: &MouseDownEvent, window, cx| {
                        window.prevent_default();
                        view.begin_git_commit_edit(window, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(display)
                .into_any_element()
        };

        let mut commit_button_bg = accent;
        if commit_message_empty && !editing_commit {
            commit_button_bg.a *= 0.35;
        }
        let mut commit_button_text = colors.background;
        commit_button_text.a = 0.95;
        let commit_area = div()
            .id("git-panel-commit-area")
            .flex_none()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(8.0))
            .border_t_1()
            .border_color(stroke)
            .child(
                div()
                    .w_full()
                    .rounded(px(5.0))
                    .bg(field_bg)
                    .when(editing_commit, |field| {
                        field.border_1().border_color(accent)
                    })
                    .child(commit_message),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(text_muted)
                            .font_family(ui_font_family.clone())
                            .child(format!("{staged_count} staged")),
                    )
                    .child(
                        div()
                            .id("git-panel-commit-button")
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(5.0))
                            .bg(commit_button_bg)
                            .text_size(px(11.0))
                            .text_color(commit_button_text)
                            .font_family(ui_font_family.clone())
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|view, _event: &MouseDownEvent, _window, cx| {
                                    view.git_panel_commit(cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .child(commit_label),
                    ),
            );

        div()
            .id("git-panel")
            .flex_none()
            .w(px(GIT_PANEL_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(stroke)
            .child(header)
            .child(summary)
            .child(
                div()
                    .id("git-panel-rows-viewport")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.git_panel.list_scroll)
                    .child(rows),
            )
            .child(commit_area)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_status_maps_codes_and_stage_states() {
        let output = "M  staged.rs\0 M unstaged.rs\0MM partial.rs\0A  added.rs\0 D deleted.rs\0?? untracked.rs\0UU conflicted.rs\0";
        let entries = parse_porcelain_status(output);
        let by_path = |path: &str| {
            entries
                .iter()
                .find(|entry| entry.path == path)
                .unwrap_or_else(|| panic!("missing {path}"))
        };

        assert_eq!(by_path("staged.rs").stage, GitStageState::Staged);
        assert_eq!(by_path("staged.rs").status, GitFileStatus::Modified);
        assert_eq!(by_path("unstaged.rs").stage, GitStageState::Unstaged);
        assert_eq!(by_path("partial.rs").stage, GitStageState::Partial);
        assert_eq!(by_path("added.rs").status, GitFileStatus::Added);
        assert_eq!(by_path("deleted.rs").status, GitFileStatus::Deleted);
        assert_eq!(by_path("untracked.rs").status, GitFileStatus::Untracked);
        assert_eq!(by_path("conflicted.rs").status, GitFileStatus::Conflicted);
    }

    #[test]
    fn porcelain_status_consumes_rename_original_path_and_sorts_untracked_last() {
        let output = "R  new_name.rs\0old_name.rs\0?? zz_untracked.rs\0M  aa_tracked.rs\0";
        let entries = parse_porcelain_status(output);
        let paths = entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["aa_tracked.rs", "new_name.rs", "zz_untracked.rs"]);
        assert_eq!(entries[1].status, GitFileStatus::Renamed);
    }

    #[test]
    fn numstat_merges_staged_and_unstaged_counts() {
        let mut counts = HashMap::new();
        merge_numstat(&mut counts, "3\t1\tsrc/lib.rs\n-\t-\tbinary.png\n");
        merge_numstat(&mut counts, "2\t4\tsrc/lib.rs\n");
        assert_eq!(counts.get("src/lib.rs").copied(), Some((5, 5)));
        assert_eq!(counts.get("binary.png"), None);
    }

    #[test]
    fn numstat_rename_paths_normalize_to_new_name() {
        assert_eq!(
            normalize_numstat_path("src/{old.rs => new.rs}"),
            "src/new.rs"
        );
        assert_eq!(normalize_numstat_path("old.rs => new.rs"), "new.rs");
        assert_eq!(
            normalize_numstat_path("crates/{a => b}/src/lib.rs"),
            "crates/b/src/lib.rs"
        );
        assert_eq!(normalize_numstat_path("plain/path.rs"), "plain/path.rs");
    }

    #[test]
    fn file_entry_splits_name_and_dir_hint() {
        let entry = GitFileEntry {
            path: "crates/desktop_app/src/main.rs".to_string(),
            status: GitFileStatus::Modified,
            stage: GitStageState::Unstaged,
            additions: None,
            deletions: None,
        };
        assert_eq!(entry.file_name(), "main.rs");
        assert_eq!(entry.dir_hint(), Some("crates/desktop_app/src"));

        let root_entry = GitFileEntry {
            path: "Cargo.lock".to_string(),
            ..entry
        };
        assert_eq!(root_entry.file_name(), "Cargo.lock");
        assert_eq!(root_entry.dir_hint(), None);
    }

    #[test]
    fn snapshot_totals_and_staged_count() {
        let snapshot = GitStatusSnapshot {
            repo_root: Some(PathBuf::from("/repo")),
            branch: "main".to_string(),
            entries: vec![
                GitFileEntry {
                    path: "a.rs".to_string(),
                    status: GitFileStatus::Modified,
                    stage: GitStageState::Staged,
                    additions: Some(10),
                    deletions: Some(2),
                },
                GitFileEntry {
                    path: "b.rs".to_string(),
                    status: GitFileStatus::Modified,
                    stage: GitStageState::Partial,
                    additions: Some(5),
                    deletions: None,
                },
                GitFileEntry {
                    path: "c.rs".to_string(),
                    status: GitFileStatus::Untracked,
                    stage: GitStageState::Unstaged,
                    additions: None,
                    deletions: None,
                },
            ],
            error: None,
        };
        assert_eq!(snapshot.total_additions(), 15);
        assert_eq!(snapshot.total_deletions(), 2);
        assert_eq!(snapshot.staged_count(), 2);
    }
}
