#![allow(dead_code)]

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimingSnapshot {
    pub count: u64,
    pub total_ns: u64,
    pub last_ns: u64,
    pub max_ns: u64,
}

impl TimingSnapshot {
    pub fn average_ns(self) -> u64 {
        match self.count {
            0 => 0,
            count => self.total_ns / count,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewPerfSnapshot {
    pub render: TimingSnapshot,
    pub expression_eval: TimingSnapshot,
    pub chart_render: TimingSnapshot,
    pub chart_widget_render: TimingSnapshot,
    pub chart_image_render: TimingSnapshot,
    pub chart_worker_total: TimingSnapshot,
    pub chart_worker_plot: TimingSnapshot,
    pub chart_resize: TimingSnapshot,
    pub requests_queued: u64,
    pub requests_drained: u64,
    pub cache_hits: u64,
    pub debounce_skips: u64,
    pub direct_widget_renders: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MultiChartPerfSnapshot {
    pub expression_eval: TimingSnapshot,
    pub detail_refresh: TimingSnapshot,
    pub dependent_recompute: TimingSnapshot,
    pub load_worker_total: TimingSnapshot,
    pub render_worker_total: TimingSnapshot,
    pub detail_items_seen: u64,
    pub load_batches: u64,
    pub load_requests_seen: u64,
    pub load_requests_coalesced: u64,
    pub render_requests_queued: u64,
    pub render_requests_drained: u64,
    pub stale_render_results: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiPerfSnapshot {
    pub preview: PreviewPerfSnapshot,
    pub mchart: MultiChartPerfSnapshot,
}

pub(crate) struct TimerGuard {
    started_at: Instant,
    metric: &'static TimingMetric,
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        self.metric.record(self.started_at.elapsed());
    }
}

pub(crate) struct TimingMetric {
    count: AtomicU64,
    total_ns: AtomicU64,
    last_ns: AtomicU64,
    max_ns: AtomicU64,
}

impl TimingMetric {
    const fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            last_ns: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
        }
    }

    pub(crate) fn start(&'static self) -> TimerGuard {
        TimerGuard {
            started_at: Instant::now(),
            metric: self,
        }
    }

    pub(crate) fn record(&self, duration: Duration) {
        let elapsed_ns = duration.as_nanos().min(u64::MAX as u128) as u64;
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        self.last_ns.store(elapsed_ns, Ordering::Relaxed);
        let mut current_max = self.max_ns.load(Ordering::Relaxed);
        while elapsed_ns > current_max {
            match self.max_ns.compare_exchange_weak(
                current_max,
                elapsed_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current_max = observed,
            }
        }
    }

    fn snapshot(&self) -> TimingSnapshot {
        TimingSnapshot {
            count: self.count.load(Ordering::Relaxed),
            total_ns: self.total_ns.load(Ordering::Relaxed),
            last_ns: self.last_ns.load(Ordering::Relaxed),
            max_ns: self.max_ns.load(Ordering::Relaxed),
        }
    }

    #[cfg(test)]
    fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.total_ns.store(0, Ordering::Relaxed);
        self.last_ns.store(0, Ordering::Relaxed);
        self.max_ns.store(0, Ordering::Relaxed);
    }
}

pub(crate) struct CounterMetric {
    value: AtomicU64,
}

impl CounterMetric {
    const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    pub(crate) fn increment(&self) {
        self.add(1);
    }

    pub(crate) fn add(&self, value: u64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    fn snapshot(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

pub(crate) struct PreviewPerfMetrics {
    pub(crate) render: TimingMetric,
    pub(crate) expression_eval: TimingMetric,
    pub(crate) chart_render: TimingMetric,
    pub(crate) chart_widget_render: TimingMetric,
    pub(crate) chart_image_render: TimingMetric,
    pub(crate) chart_worker_total: TimingMetric,
    pub(crate) chart_worker_plot: TimingMetric,
    pub(crate) chart_resize: TimingMetric,
    pub(crate) requests_queued: CounterMetric,
    pub(crate) requests_drained: CounterMetric,
    pub(crate) cache_hits: CounterMetric,
    pub(crate) debounce_skips: CounterMetric,
    pub(crate) direct_widget_renders: CounterMetric,
}

impl PreviewPerfMetrics {
    const fn new() -> Self {
        Self {
            render: TimingMetric::new(),
            expression_eval: TimingMetric::new(),
            chart_render: TimingMetric::new(),
            chart_widget_render: TimingMetric::new(),
            chart_image_render: TimingMetric::new(),
            chart_worker_total: TimingMetric::new(),
            chart_worker_plot: TimingMetric::new(),
            chart_resize: TimingMetric::new(),
            requests_queued: CounterMetric::new(),
            requests_drained: CounterMetric::new(),
            cache_hits: CounterMetric::new(),
            debounce_skips: CounterMetric::new(),
            direct_widget_renders: CounterMetric::new(),
        }
    }

    fn snapshot(&self) -> PreviewPerfSnapshot {
        PreviewPerfSnapshot {
            render: self.render.snapshot(),
            expression_eval: self.expression_eval.snapshot(),
            chart_render: self.chart_render.snapshot(),
            chart_widget_render: self.chart_widget_render.snapshot(),
            chart_image_render: self.chart_image_render.snapshot(),
            chart_worker_total: self.chart_worker_total.snapshot(),
            chart_worker_plot: self.chart_worker_plot.snapshot(),
            chart_resize: self.chart_resize.snapshot(),
            requests_queued: self.requests_queued.snapshot(),
            requests_drained: self.requests_drained.snapshot(),
            cache_hits: self.cache_hits.snapshot(),
            debounce_skips: self.debounce_skips.snapshot(),
            direct_widget_renders: self.direct_widget_renders.snapshot(),
        }
    }

    #[cfg(test)]
    fn reset(&self) {
        self.render.reset();
        self.expression_eval.reset();
        self.chart_render.reset();
        self.chart_widget_render.reset();
        self.chart_image_render.reset();
        self.chart_worker_total.reset();
        self.chart_worker_plot.reset();
        self.chart_resize.reset();
        self.requests_queued.reset();
        self.requests_drained.reset();
        self.cache_hits.reset();
        self.debounce_skips.reset();
        self.direct_widget_renders.reset();
    }
}

pub(crate) struct MultiChartPerfMetrics {
    pub(crate) expression_eval: TimingMetric,
    pub(crate) detail_refresh: TimingMetric,
    pub(crate) dependent_recompute: TimingMetric,
    pub(crate) load_worker_total: TimingMetric,
    pub(crate) render_worker_total: TimingMetric,
    pub(crate) detail_items_seen: CounterMetric,
    pub(crate) load_batches: CounterMetric,
    pub(crate) load_requests_seen: CounterMetric,
    pub(crate) load_requests_coalesced: CounterMetric,
    pub(crate) render_requests_queued: CounterMetric,
    pub(crate) render_requests_drained: CounterMetric,
    pub(crate) stale_render_results: CounterMetric,
}

impl MultiChartPerfMetrics {
    const fn new() -> Self {
        Self {
            expression_eval: TimingMetric::new(),
            detail_refresh: TimingMetric::new(),
            dependent_recompute: TimingMetric::new(),
            load_worker_total: TimingMetric::new(),
            render_worker_total: TimingMetric::new(),
            detail_items_seen: CounterMetric::new(),
            load_batches: CounterMetric::new(),
            load_requests_seen: CounterMetric::new(),
            load_requests_coalesced: CounterMetric::new(),
            render_requests_queued: CounterMetric::new(),
            render_requests_drained: CounterMetric::new(),
            stale_render_results: CounterMetric::new(),
        }
    }

    fn snapshot(&self) -> MultiChartPerfSnapshot {
        MultiChartPerfSnapshot {
            expression_eval: self.expression_eval.snapshot(),
            detail_refresh: self.detail_refresh.snapshot(),
            dependent_recompute: self.dependent_recompute.snapshot(),
            load_worker_total: self.load_worker_total.snapshot(),
            render_worker_total: self.render_worker_total.snapshot(),
            detail_items_seen: self.detail_items_seen.snapshot(),
            load_batches: self.load_batches.snapshot(),
            load_requests_seen: self.load_requests_seen.snapshot(),
            load_requests_coalesced: self.load_requests_coalesced.snapshot(),
            render_requests_queued: self.render_requests_queued.snapshot(),
            render_requests_drained: self.render_requests_drained.snapshot(),
            stale_render_results: self.stale_render_results.snapshot(),
        }
    }

    #[cfg(test)]
    fn reset(&self) {
        self.expression_eval.reset();
        self.detail_refresh.reset();
        self.dependent_recompute.reset();
        self.load_worker_total.reset();
        self.render_worker_total.reset();
        self.detail_items_seen.reset();
        self.load_batches.reset();
        self.load_requests_seen.reset();
        self.load_requests_coalesced.reset();
        self.render_requests_queued.reset();
        self.render_requests_drained.reset();
        self.stale_render_results.reset();
    }
}

pub(crate) struct UiPerfMetrics {
    pub(crate) preview: PreviewPerfMetrics,
    pub(crate) mchart: MultiChartPerfMetrics,
}

impl UiPerfMetrics {
    const fn new() -> Self {
        Self {
            preview: PreviewPerfMetrics::new(),
            mchart: MultiChartPerfMetrics::new(),
        }
    }

    #[cfg(test)]
    fn reset(&self) {
        self.preview.reset();
        self.mchart.reset();
    }
}

static METRICS: OnceLock<UiPerfMetrics> = OnceLock::new();

pub(crate) fn metrics() -> &'static UiPerfMetrics {
    METRICS.get_or_init(UiPerfMetrics::new)
}

pub(crate) fn snapshot() -> UiPerfSnapshot {
    let metrics = metrics();
    UiPerfSnapshot {
        preview: metrics.preview.snapshot(),
        mchart: metrics.mchart.snapshot(),
    }
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    metrics().reset();
}

#[cfg(test)]
mod tests {
    use super::{metrics, reset_for_tests, snapshot};
    use std::time::Duration;

    #[test]
    fn timing_snapshot_tracks_count_total_last_and_max() {
        reset_for_tests();

        metrics()
            .preview
            .chart_render
            .record(Duration::from_micros(10));
        metrics()
            .preview
            .chart_render
            .record(Duration::from_micros(25));

        let snapshot = snapshot();
        assert_eq!(snapshot.preview.chart_render.count, 2);
        assert_eq!(snapshot.preview.chart_render.last_ns, 25_000);
        assert_eq!(snapshot.preview.chart_render.max_ns, 25_000);
        assert_eq!(snapshot.preview.chart_render.total_ns, 35_000);
        assert_eq!(snapshot.preview.chart_render.average_ns(), 17_500);
    }

    #[test]
    fn counters_accumulate_independent_churn_metrics() {
        reset_for_tests();

        metrics().preview.requests_queued.add(2);
        metrics().preview.requests_drained.increment();
        metrics().mchart.load_requests_seen.add(3);
        metrics().mchart.load_requests_coalesced.add(2);

        let snapshot = snapshot();
        assert_eq!(snapshot.preview.requests_queued, 2);
        assert_eq!(snapshot.preview.requests_drained, 1);
        assert_eq!(snapshot.mchart.load_requests_seen, 3);
        assert_eq!(snapshot.mchart.load_requests_coalesced, 2);
    }
}
