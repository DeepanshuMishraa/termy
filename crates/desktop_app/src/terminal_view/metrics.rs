use super::constants::DEBUG_OVERLAY_SAMPLE_INTERVAL;
#[cfg(debug_assertions)]
use super::constants::RENDER_METRICS_LOG_INTERVAL;
use std::time::Instant;
#[cfg(debug_assertions)]
use std::{env, time::Duration};
use sysinfo::{ProcessesToUpdate, System, get_current_pid};
#[cfg(debug_assertions)]
use termy_terminal_ui::terminal_ui_render_metrics_reset;
use termy_terminal_ui::{TerminalUiRenderMetricsSnapshot, terminal_ui_render_metrics_snapshot};

#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TerminalRenderMetricsCounters {
    pub(super) render_count: u64,
    pub(super) cache_full_count: u64,
    pub(super) cache_partial_count: u64,
    pub(super) cache_reuse_count: u64,
    pub(super) dirty_span_count: u64,
    pub(super) patched_cell_count: u64,
}

#[cfg(debug_assertions)]
impl TerminalRenderMetricsCounters {
    pub(super) fn saturating_sub(self, previous: Self) -> Self {
        Self {
            render_count: self.render_count.saturating_sub(previous.render_count),
            cache_full_count: self
                .cache_full_count
                .saturating_sub(previous.cache_full_count),
            cache_partial_count: self
                .cache_partial_count
                .saturating_sub(previous.cache_partial_count),
            cache_reuse_count: self
                .cache_reuse_count
                .saturating_sub(previous.cache_reuse_count),
            dirty_span_count: self
                .dirty_span_count
                .saturating_sub(previous.dirty_span_count),
            patched_cell_count: self
                .patched_cell_count
                .saturating_sub(previous.patched_cell_count),
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub(super) struct TerminalRenderMetricsState {
    pub(super) enabled: bool,
    pub(super) counters: TerminalRenderMetricsCounters,
    pub(super) last_emit_counters: TerminalRenderMetricsCounters,
    pub(super) last_emit_terminal_ui: TerminalUiRenderMetricsSnapshot,
    pub(super) last_emit_at: Option<Instant>,
    pub(super) log_interval: Duration,
}

#[cfg(debug_assertions)]
impl TerminalRenderMetricsState {
    fn parse_env_flag(value: &str) -> bool {
        matches!(value.trim(), "1")
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("on")
    }

    fn enabled_from_env() -> bool {
        env::var("TERMY_RENDER_METRICS")
            .ok()
            .is_some_and(|value| Self::parse_env_flag(value.as_str()))
    }

    pub(super) fn from_env() -> Self {
        let enabled = Self::enabled_from_env();
        if enabled {
            terminal_ui_render_metrics_reset();
        }
        Self {
            enabled,
            counters: TerminalRenderMetricsCounters::default(),
            last_emit_counters: TerminalRenderMetricsCounters::default(),
            last_emit_terminal_ui: terminal_ui_render_metrics_snapshot(),
            last_emit_at: None,
            log_interval: RENDER_METRICS_LOG_INTERVAL,
        }
    }
}

#[derive(Debug)]
pub(super) struct DebugOverlayStats {
    system: System,
    pid: Option<sysinfo::Pid>,
    sample_started_at: Instant,
    last_frame_at: Option<Instant>,
    frames_in_sample: u32,
    frame_interval_samples_micros: Vec<u32>,
    pub(super) fps: f32,
    pub(super) frame_p50_ms: f32,
    pub(super) frame_p95_ms: f32,
    pub(super) frame_p99_ms: f32,
    pub(super) cpu_percent: f32,
    pub(super) memory_bytes: u64,
    pub(super) view_wake_signals: u64,
    pub(super) terminal_event_drain_passes: u64,
    pub(super) terminal_redraws: u64,
    pub(super) alt_screen_fallback_redraws: u64,
    pub(super) span_damage_ms: f32,
    pub(super) span_rebuild_ms: f32,
    pub(super) span_shaping_ms: f32,
    pub(super) span_paint_ms: f32,
    span_snapshot_base: TerminalUiRenderMetricsSnapshot,
    #[cfg(debug_assertions)]
    runtime_wakeup_base: u64,
    #[cfg(debug_assertions)]
    pub(super) runtime_wakeups: u64,
}

impl DebugOverlayStats {
    pub(super) fn new() -> Self {
        #[cfg(debug_assertions)]
        let runtime_wakeup_base = terminal_ui_render_metrics_snapshot().runtime_wakeup_count;
        let mut stats = Self {
            system: System::new(),
            pid: get_current_pid().ok(),
            sample_started_at: Instant::now(),
            last_frame_at: None,
            frames_in_sample: 0,
            frame_interval_samples_micros: Vec::with_capacity(128),
            fps: 0.0,
            frame_p50_ms: 0.0,
            frame_p95_ms: 0.0,
            frame_p99_ms: 0.0,
            cpu_percent: 0.0,
            memory_bytes: 0,
            view_wake_signals: 0,
            terminal_event_drain_passes: 0,
            terminal_redraws: 0,
            alt_screen_fallback_redraws: 0,
            span_damage_ms: 0.0,
            span_rebuild_ms: 0.0,
            span_shaping_ms: 0.0,
            span_paint_ms: 0.0,
            span_snapshot_base: terminal_ui_render_metrics_snapshot(),
            #[cfg(debug_assertions)]
            runtime_wakeup_base,
            #[cfg(debug_assertions)]
            runtime_wakeups: 0,
        };
        stats.refresh_process_metrics();
        stats.refresh_runtime_wakeups();
        stats
    }

    pub(super) fn reset(&mut self) {
        self.sample_started_at = Instant::now();
        self.last_frame_at = None;
        self.frames_in_sample = 0;
        self.frame_interval_samples_micros.clear();
        self.fps = 0.0;
        self.frame_p50_ms = 0.0;
        self.frame_p95_ms = 0.0;
        self.frame_p99_ms = 0.0;
        self.view_wake_signals = 0;
        self.terminal_event_drain_passes = 0;
        self.terminal_redraws = 0;
        self.alt_screen_fallback_redraws = 0;
        self.span_damage_ms = 0.0;
        self.span_rebuild_ms = 0.0;
        self.span_shaping_ms = 0.0;
        self.span_paint_ms = 0.0;
        self.span_snapshot_base = terminal_ui_render_metrics_snapshot();
        #[cfg(debug_assertions)]
        {
            self.runtime_wakeup_base = terminal_ui_render_metrics_snapshot().runtime_wakeup_count;
            self.runtime_wakeups = 0;
        }
        self.refresh_process_metrics();
    }

    pub(super) fn record_frame(&mut self, now: Instant) {
        if let Some(previous_frame_at) = self.last_frame_at.replace(now) {
            let frame_interval = now.saturating_duration_since(previous_frame_at);
            let micros = frame_interval.as_micros().min(u128::from(u32::MAX)) as u32;
            self.frame_interval_samples_micros.push(micros);
        }
        self.frames_in_sample = self.frames_in_sample.saturating_add(1);
        let elapsed = now.saturating_duration_since(self.sample_started_at);
        if elapsed < DEBUG_OVERLAY_SAMPLE_INTERVAL {
            return;
        }

        let elapsed_secs = elapsed.as_secs_f32();
        if elapsed_secs > f32::EPSILON {
            self.fps = self.frames_in_sample as f32 / elapsed_secs;
        }
        self.refresh_frame_percentiles();
        self.refresh_span_timings();
        self.refresh_runtime_wakeups();
        self.sample_started_at = now;
        self.frames_in_sample = 0;
        self.frame_interval_samples_micros.clear();
        self.refresh_process_metrics();
    }

    pub(super) fn record_view_wake_signal(&mut self) {
        self.view_wake_signals = self.view_wake_signals.saturating_add(1);
    }

    pub(super) fn record_terminal_event_drain_pass(&mut self) {
        self.terminal_event_drain_passes = self.terminal_event_drain_passes.saturating_add(1);
    }

    pub(super) fn record_terminal_redraw(&mut self) {
        self.terminal_redraws = self.terminal_redraws.saturating_add(1);
    }

    #[allow(dead_code)]
    pub(super) fn record_alt_screen_fallback_redraw(&mut self) {
        self.alt_screen_fallback_redraws = self.alt_screen_fallback_redraws.saturating_add(1);
    }

    fn refresh_process_metrics(&mut self) {
        let Some(pid) = self.pid else {
            self.cpu_percent = 0.0;
            self.memory_bytes = 0;
            return;
        };

        let _ = self
            .system
            .refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        if let Some(process) = self.system.process(pid) {
            self.cpu_percent = process.cpu_usage();
            self.memory_bytes = process.memory();
        }
    }

    fn refresh_frame_percentiles(&mut self) {
        if self.frame_interval_samples_micros.is_empty() {
            self.frame_p50_ms = 0.0;
            self.frame_p95_ms = 0.0;
            self.frame_p99_ms = 0.0;
            return;
        }

        let mut sorted_samples = self.frame_interval_samples_micros.clone();
        sorted_samples.sort_unstable();
        self.frame_p50_ms = percentile_millis(&sorted_samples, 50, 100);
        self.frame_p95_ms = percentile_millis(&sorted_samples, 95, 100);
        self.frame_p99_ms = percentile_millis(&sorted_samples, 99, 100);
    }

    fn refresh_span_timings(&mut self) {
        let current = terminal_ui_render_metrics_snapshot();
        let delta = current.saturating_sub(self.span_snapshot_base);
        self.span_snapshot_base = current;
        let frames = self.frames_in_sample.max(1) as f32;
        self.span_damage_ms = delta.span_damage_compute_us as f32 / 1000.0 / frames;
        self.span_rebuild_ms = delta.span_row_ops_rebuild_us as f32 / 1000.0 / frames;
        self.span_shaping_ms = delta.span_text_shaping_us as f32 / 1000.0 / frames;
        self.span_paint_ms = delta.span_grid_paint_us as f32 / 1000.0 / frames;
    }

    #[cfg(debug_assertions)]
    fn refresh_runtime_wakeups(&mut self) {
        let snapshot = terminal_ui_render_metrics_snapshot();
        self.runtime_wakeups = snapshot
            .runtime_wakeup_count
            .saturating_sub(self.runtime_wakeup_base);
    }

    #[cfg(not(debug_assertions))]
    fn refresh_runtime_wakeups(&mut self) {}
}

fn percentile_millis(samples_micros: &[u32], numerator: usize, denominator: usize) -> f32 {
    let Some(last_index) = samples_micros.len().checked_sub(1) else {
        return 0.0;
    };
    let rank = (samples_micros.len().saturating_mul(numerator) + denominator.saturating_sub(1))
        / denominator;
    let index = rank.saturating_sub(1).min(last_index);
    samples_micros[index] as f32 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(debug_assertions)]
    #[test]
    fn render_metrics_env_parser_accepts_truthy_values() {
        assert!(TerminalRenderMetricsState::parse_env_flag("1"));
        assert!(TerminalRenderMetricsState::parse_env_flag("true"));
        assert!(TerminalRenderMetricsState::parse_env_flag("TRUE"));
        assert!(TerminalRenderMetricsState::parse_env_flag("yes"));
        assert!(TerminalRenderMetricsState::parse_env_flag("on"));
    }

    #[test]
    fn percentile_millis_uses_full_length_rank() {
        let samples: Vec<u32> = (1..=100).collect();

        assert_eq!(percentile_millis(&samples, 50, 100), 0.050);
        assert_eq!(percentile_millis(&samples, 95, 100), 0.095);
        assert_eq!(percentile_millis(&samples, 99, 100), 0.099);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn render_metrics_env_parser_rejects_empty_and_zero_values() {
        assert!(!TerminalRenderMetricsState::parse_env_flag(""));
        assert!(!TerminalRenderMetricsState::parse_env_flag("0"));
        assert!(!TerminalRenderMetricsState::parse_env_flag("false"));
    }
}
