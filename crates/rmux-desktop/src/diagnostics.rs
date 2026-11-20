//! Opt-in, bounded counters. No timers, frame logs or automatic file writes.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT_PANE: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct ExportState(pub bool);
impl gpui::Global for ExportState {}

pub fn destination() -> Option<PathBuf> {
    std::env::var_os("RMUX_RENDER_METRICS")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

#[derive(Default)]
pub struct Timing {
    count: u64,
    total_ns: u128,
    max_ns: u128,
}
impl Timing {
    pub fn record(&mut self, elapsed: Duration) {
        let ns = elapsed.as_nanos();
        self.count += 1;
        self.total_ns += ns;
        self.max_ns = self.max_ns.max(ns);
    }
}

pub struct Metrics {
    id: u64,
    started: Instant,
    pub notifications: u64,
    pub snapshot_busy: u64,
    pub metadata_busy: u64,
    pub resize_busy: u64,
    pub rows_reused: u64,
    pub rows_rebuilt: u64,
    pub cursor_glyphs_reused: u64,
    pub cursor_glyphs_rebuilt: u64,
    pub cache_releases: u64,
    pub rows_released: u64,
    pub prepaint: Timing,
    pub paint: Timing,
}
impl Metrics {
    pub fn enabled() -> Option<Self> {
        destination().map(|_| Self {
            id: NEXT_PANE.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
            notifications: 0,
            snapshot_busy: 0,
            metadata_busy: 0,
            resize_busy: 0,
            rows_reused: 0,
            rows_rebuilt: 0,
            cursor_glyphs_reused: 0,
            cursor_glyphs_rebuilt: 0,
            cache_releases: 0,
            rows_released: 0,
            prepaint: Timing::default(),
            paint: Timing::default(),
        })
    }
    pub fn snapshot(
        &self,
        timestamp_ms: u128,
        output_bytes: u64,
        focused: bool,
        active: bool,
        frame: &rmux_terminal::Frame,
        memory: Option<rmux_terminal::MemoryUsage>,
    ) -> String {
        let columns = frame.columns;
        let rows = frame.rows.len();
        let storage = frame.storage();
        let snapshot_storage = serde_json::json!({
            "cell_capacity_bytes": storage.cell_capacity_bytes,
            "text_bytes": storage.text_bytes,
            "inline_cells": storage.inline_cells,
            "heap_cells": storage.heap_cells,
            "heap_text_payload_bytes": storage.heap_text_payload_bytes,
        });
        let memory_json = memory
            .map(|m| {
                serde_json::json!({
                    "primary_resident_bytes": m.primary_resident_bytes,
                    "alternate_resident_bytes": m.alternate_resident_bytes,
                    "virtual_bytes": m.virtual_bytes,
                    "compressed_bytes": m.compressed_bytes,
                    "image_bytes": m.image_bytes,
                    "compression_supported": m.compression_supported,
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let snapshot_busy = self.snapshot_busy;
        let metadata_busy = self.metadata_busy;
        let resize_busy = self.resize_busy;
        let cache_releases = self.cache_releases;
        let rows_released = self.rows_released;
        let cursor_glyphs_reused = self.cursor_glyphs_reused;
        let cursor_glyphs_rebuilt = self.cursor_glyphs_rebuilt;
        let mut output = format!(
            "{{\"timestamp_ms\":{timestamp_ms},\"pid\":{},\"pane_id\":{},\"elapsed_ms\":{},\"columns\":{columns},\"rows\":{rows},\"pane_focused\":{focused},\"window_active\":{active},\"parsed_output_bytes\":{output_bytes},\"output_notifications\":{},\"rows_reused\":{},\"rows_rebuilt\":{},\"prepaint_calls\":{},\"prepaint_total_ns\":{},\"prepaint_max_ns\":{},\"paint_calls\":{},\"paint_total_ns\":{},\"paint_max_ns\":{},\"ghostty_memory\":{memory_json},\"cache_releases\":{cache_releases},\"rows_released\":{rows_released},\"cursor_glyphs_reused\":{cursor_glyphs_reused},\"cursor_glyphs_rebuilt\":{cursor_glyphs_rebuilt}}}\n",
            std::process::id(),
            self.id,
            self.started.elapsed().as_millis(),
            self.notifications,
            self.rows_reused,
            self.rows_rebuilt,
            self.prepaint.count,
            self.prepaint.total_ns,
            self.prepaint.max_ns,
            self.paint.count,
            self.paint.total_ns,
            self.paint.max_ns
        );
        // Append the bounded snapshot accounting object before the closing brace.
        output.truncate(output.len() - 2);
        output.push_str(&format!(",\"resize_busy\":{resize_busy},\"metadata_busy\":{metadata_busy},\"snapshot_busy\":{snapshot_busy},\"snapshot_storage\":{snapshot_storage}}}\n"));
        output
    }
}
