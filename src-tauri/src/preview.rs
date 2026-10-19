use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use tokio::sync::broadcast;

use crate::bus::EventBus;
use crate::config_loader::ConfigLoader;
use crate::error::AppError;
use crate::events::AppEvent;

pub const PREVIEW_WIDTH: u32 = 320;
pub const PREVIEW_HEIGHT: u32 = 180;
pub const PREVIEW_FPS: u32 = 8;
pub const LAYOUTS: [&str; 3] = ["2x2", "2x3", "2x4"];
const MAX_IN_FLIGHT_ENCODES: usize = 16;
const STATS_INTERVAL: Duration = Duration::from_secs(1);
const SIMULATED_SOURCE_DELAY_MS: u32 = 12;

/// A decoded thumbnail (always 320x180 RGBA) straight from a backend; `native_*` is the source's own size.
pub struct RawFrame {
    pub source: String,
    pub native_width: u32,
    pub native_height: u32,
    pub rgba: Vec<u8>,
    pub source_delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PreviewFrame {
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub png_base64: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SourceStatsSample {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PreviewSnapshot {
    pub frames: Vec<PreviewFrame>,
    pub stats: Vec<SourceStatsSample>,
}

/// Per-source rate limiter; pure so the 8 fps cap is testable without a clock.
pub struct FrameThrottle {
    min_interval: Duration,
    last: HashMap<String, Instant>,
}

impl FrameThrottle {
    pub fn new(fps: u32) -> Self {
        Self {
            min_interval: Duration::from_micros(1_000_000 / u64::from(fps.max(1))),
            last: HashMap::new(),
        }
    }

    pub fn allow(&mut self, source: &str, now: Instant) -> bool {
        match self.last.get(source) {
            Some(prev) if now.saturating_duration_since(*prev) < self.min_interval => false,
            _ => {
                self.last.insert(source.to_string(), now);
                true
            }
        }
    }

    pub fn forget(&mut self, source: &str) {
        self.last.remove(source);
    }
}

struct Window {
    start: Instant,
    frames: u32,
    width: u32,
    height: u32,
    latency_ms: u32,
}

/// Per-source fps / resolution / latency accumulator, reported once per window.
#[derive(Default)]
pub struct StatsTracker {
    sources: HashMap<String, Window>,
}

impl StatsTracker {
    pub fn record_arrival(&mut self, source: &str, now: Instant, width: u32, height: u32) {
        let w = self.sources.entry(source.to_string()).or_insert(Window {
            start: now,
            frames: 0,
            width,
            height,
            latency_ms: 0,
        });
        w.frames += 1;
        w.width = width;
        w.height = height;
    }

    pub fn record_latency(&mut self, source: &str, latency_ms: u32) {
        if let Some(w) = self.sources.get_mut(source) {
            w.latency_ms = latency_ms;
        }
    }

    pub fn forget(&mut self, source: &str) {
        self.sources.remove(source);
    }

    /// One sample per source for the window ending at `now`; starts a fresh window.
    pub fn report(&mut self, now: Instant) -> Vec<SourceStatsSample> {
        let mut out: Vec<SourceStatsSample> = self
            .sources
            .iter_mut()
            .map(|(name, w)| {
                let secs = now
                    .saturating_duration_since(w.start)
                    .as_secs_f32()
                    .max(0.001);
                let sample = SourceStatsSample {
                    name: name.clone(),
                    width: w.width,
                    height: w.height,
                    fps: (w.frames as f32 / secs * 10.0).round() / 10.0,
                    latency_ms: w.latency_ms,
                };
                w.start = now;
                w.frames = 0;
                sample
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

pub fn encode_png_base64(rgba: &[u8], width: u32, height: u32) -> Result<String, AppError> {
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err(AppError::InvalidInput(
            "frame size does not match RGBA dimensions".into(),
        ));
    }
    let mut png_bytes = Vec::with_capacity(rgba.len() / 4);
    let fail = |e: png::EncodingError| AppError::Internal(format!("PNG encode failed: {e}"));
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(fail)?;
        writer.write_image_data(rgba).map_err(fail)?;
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(png_bytes))
}

/// Deterministic 320x180 RGBA test pattern: a per-source colour field with a bar that moves each frame.
pub fn test_pattern(source: &str, frame_no: u64) -> Vec<u8> {
    let hash = source.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    });
    let (r, g, b) = (hash as u8, (hash >> 8) as u8, (hash >> 16) as u8);
    let bar = (frame_no * 8 % u64::from(PREVIEW_WIDTH)) as u32;
    let mut out = Vec::with_capacity((PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize);
    for _y in 0..PREVIEW_HEIGHT {
        for x in 0..PREVIEW_WIDTH {
            if x >= bar && x < bar + 8 {
                out.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    out
}

struct Inner {
    throttle: Mutex<FrameThrottle>,
    stats: Mutex<StatsTracker>,
    latest_frames: Mutex<HashMap<String, PreviewFrame>>,
    latest_stats: Mutex<HashMap<String, SourceStatsSample>>,
    frames: broadcast::Sender<PreviewFrame>,
    in_flight: AtomicUsize,
}

/// Throttles, encodes (off the caller's thread) and fans out preview frames; cheap to clone.
#[derive(Clone)]
pub struct PreviewHub(Arc<Inner>);

impl Default for PreviewHub {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewHub {
    pub fn new() -> Self {
        let (frames, _) = broadcast::channel(64);
        Self(Arc::new(Inner {
            throttle: Mutex::new(FrameThrottle::new(PREVIEW_FPS)),
            stats: Mutex::new(StatsTracker::default()),
            latest_frames: Mutex::new(HashMap::new()),
            latest_stats: Mutex::new(HashMap::new()),
            frames,
            in_flight: AtomicUsize::new(0),
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PreviewFrame> {
        self.0.frames.subscribe()
    }

    /// Called from any backend thread; never blocks on encoding.
    pub fn push(&self, frame: RawFrame) {
        let size_ok = frame.rgba.len() == (PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize;
        if !size_ok {
            return;
        }
        let now = Instant::now();
        if let Ok(mut s) = self.0.stats.lock() {
            s.record_arrival(&frame.source, now, frame.native_width, frame.native_height);
        }
        let allowed = self
            .0
            .throttle
            .lock()
            .map(|mut t| t.allow(&frame.source, now));
        if !matches!(allowed, Ok(true)) {
            return;
        }
        if self.0.in_flight.fetch_add(1, Ordering::AcqRel) >= MAX_IN_FLIGHT_ENCODES {
            self.0.in_flight.fetch_sub(1, Ordering::AcqRel);
            return;
        }
        let inner = Arc::clone(&self.0);
        tauri::async_runtime::spawn_blocking(move || {
            let encoded = encode_png_base64(&frame.rgba, PREVIEW_WIDTH, PREVIEW_HEIGHT);
            inner.in_flight.fetch_sub(1, Ordering::AcqRel);
            let Ok(png_base64) = encoded else { return };
            let latency = frame.source_delay_ms + now.elapsed().as_millis() as u32;
            if let Ok(mut s) = inner.stats.lock() {
                s.record_latency(&frame.source, latency);
            }
            let out = PreviewFrame {
                source: frame.source,
                width: PREVIEW_WIDTH,
                height: PREVIEW_HEIGHT,
                png_base64,
            };
            if let Ok(mut latest) = inner.latest_frames.lock() {
                latest.insert(out.source.clone(), out.clone());
            }
            let _ = inner.frames.send(out);
        });
    }

    /// Drops all per-source state when a source is detached.
    pub fn forget(&self, source: &str) {
        if let Ok(mut t) = self.0.throttle.lock() {
            t.forget(source);
        }
        if let Ok(mut s) = self.0.stats.lock() {
            s.forget(source);
        }
        if let Ok(mut f) = self.0.latest_frames.lock() {
            f.remove(source);
        }
        if let Ok(mut s) = self.0.latest_stats.lock() {
            s.remove(source);
        }
    }

    /// Publishes one `SourceStats` per attached source; the 1s task calls this.
    pub fn flush_stats(&self, bus: &EventBus, now: Instant) {
        let samples = match self.0.stats.lock() {
            Ok(mut s) => s.report(now),
            Err(_) => return,
        };
        for sample in samples {
            if let Ok(mut latest) = self.0.latest_stats.lock() {
                latest.insert(sample.name.clone(), sample.clone());
            }
            bus.publish(AppEvent::SourceStats {
                name: sample.name,
                width: sample.width,
                height: sample.height,
                fps: sample.fps,
                latency_ms: sample.latency_ms,
            });
        }
    }

    /// Latest frame and stats per source, so a re-subscribing UI (after sleep/wake) needs no event history.
    pub fn snapshot(&self) -> PreviewSnapshot {
        let mut frames: Vec<_> = self
            .0
            .latest_frames
            .lock()
            .map(|f| f.values().cloned().collect())
            .unwrap_or_default();
        frames.sort_by(|a, b| a.source.cmp(&b.source));
        let mut stats: Vec<_> = self
            .0
            .latest_stats
            .lock()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default();
        stats.sort_by(|a, b| a.name.cmp(&b.name));
        PreviewSnapshot { frames, stats }
    }
}

/// Publishes per-source stats once a second.
pub fn spawn_stats_task(
    hub: PreviewHub,
    bus: Arc<EventBus>,
) -> tauri::async_runtime::JoinHandle<()> {
    // Called from .setup(), outside any Tokio runtime — tokio::spawn would panic here.
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(STATS_INTERVAL);
        loop {
            interval.tick().await;
            hub.flush_stats(&bus, Instant::now());
        }
    })
}

pub fn validate_layout(layout: &str) -> Result<(), AppError> {
    if LAYOUTS.contains(&layout) {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "unknown preview layout: {layout}"
        )))
    }
}

pub fn get_layout(config_path: &Path) -> Result<String, AppError> {
    let config = ConfigLoader::load_or_default(&config_path.to_path_buf())?;
    Ok(config.ui.preview_grid)
}

pub fn set_layout(config_path: &Path, layout: &str) -> Result<(), AppError> {
    validate_layout(layout)?;
    let path = config_path.to_path_buf();
    let mut config = ConfigLoader::load_or_default(&path)?;
    config.ui.preview_grid = layout.to_string();
    ConfigLoader::save(&path, &config)
}

/// Generates synthetic frames per attached source for the simulated backend (30 fps by default).
pub struct SimulatedFeeds {
    hub: PreviewHub,
    fps: u32,
    workers: HashMap<String, (Arc<AtomicBool>, std::thread::JoinHandle<()>)>,
}

impl SimulatedFeeds {
    pub fn new(hub: PreviewHub, fps: u32) -> Self {
        Self {
            hub,
            fps: fps.max(1),
            workers: HashMap::new(),
        }
    }

    pub fn start(&mut self, source: &str) {
        self.stop(source);
        let stop = Arc::new(AtomicBool::new(false));
        let (hub, name, flag, fps) = (self.hub.clone(), source.to_string(), stop.clone(), self.fps);
        let spawned = std::thread::Builder::new()
            .name("sim-feed".into())
            .spawn(move || {
                let tick = Duration::from_micros(1_000_000 / u64::from(fps));
                let mut n = 0u64;
                while !flag.load(Ordering::Acquire) {
                    hub.push(RawFrame {
                        source: name.clone(),
                        native_width: 1920,
                        native_height: 1080,
                        rgba: test_pattern(&name, n),
                        source_delay_ms: SIMULATED_SOURCE_DELAY_MS,
                    });
                    n += 1;
                    std::thread::sleep(tick);
                }
            });
        if let Ok(handle) = spawned {
            self.workers.insert(source.to_string(), (stop, handle));
        }
    }

    pub fn stop(&mut self, source: &str) {
        if let Some((flag, handle)) = self.workers.remove(source) {
            flag.store(true, Ordering::Release);
            let _ = handle.join();
        }
        self.hub.forget(source);
    }

    pub fn stop_all(&mut self) {
        let names: Vec<String> = self.workers.keys().cloned().collect();
        for n in names {
            self.stop(&n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_caps_a_fast_source_at_8_fps_per_source() {
        let mut t = FrameThrottle::new(PREVIEW_FPS);
        let t0 = Instant::now();
        let (mut a, mut b) = (0, 0);
        for ms in 0..10_000u64 {
            let now = t0 + Duration::from_millis(ms);
            if t.allow("a", now) {
                a += 1;
            }
            if t.allow("b", now) {
                b += 1;
            }
        }
        assert!((70..=80).contains(&a), "a: {a}");
        assert_eq!(a, b);
    }

    #[test]
    fn throttle_passes_the_first_frame_and_slow_sources_untouched() {
        let mut t = FrameThrottle::new(8);
        let t0 = Instant::now();
        assert!(t.allow("a", t0));
        assert!(!t.allow("a", t0 + Duration::from_millis(100)));
        assert!(t.allow("a", t0 + Duration::from_millis(125)));
        assert!(t.allow("a", t0 + Duration::from_millis(400)));
    }

    #[test]
    fn throttle_forget_lets_a_reattached_source_through_immediately() {
        let mut t = FrameThrottle::new(8);
        let t0 = Instant::now();
        assert!(t.allow("a", t0));
        t.forget("a");
        assert!(t.allow("a", t0 + Duration::from_millis(1)));
    }

    #[test]
    fn stats_report_fps_resolution_and_latency_then_reset_the_window() {
        let mut s = StatsTracker::default();
        let t0 = Instant::now();
        for i in 0..30 {
            s.record_arrival("a", t0 + Duration::from_millis(i * 33), 1920, 1080);
        }
        s.record_latency("a", 42);
        let out = s.report(t0 + Duration::from_secs(1));
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].width, out[0].height), (1920, 1080));
        assert!((out[0].fps - 30.0).abs() < 0.11, "fps {}", out[0].fps);
        assert_eq!(out[0].latency_ms, 42);
        let idle = s.report(t0 + Duration::from_secs(2));
        assert_eq!(idle[0].fps, 0.0);
        assert_eq!(idle[0].latency_ms, 42);
    }

    #[test]
    fn stats_forget_drops_the_source() {
        let mut s = StatsTracker::default();
        s.record_arrival("a", Instant::now(), 1, 1);
        s.forget("a");
        assert!(s.report(Instant::now()).is_empty());
    }

    #[test]
    fn test_pattern_is_deterministic_correctly_sized_and_distinct() {
        let a0 = test_pattern("Cam A", 0);
        assert_eq!(a0.len(), (PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize);
        assert_eq!(a0, test_pattern("Cam A", 0));
        assert_ne!(a0, test_pattern("Cam A", 1));
        assert_ne!(a0, test_pattern("Cam B", 0));
    }

    #[test]
    fn png_round_trips_to_the_original_pixels() {
        let rgba = test_pattern("Cam A", 3);
        let b64 = encode_png_base64(&rgba, PREVIEW_WIDTH, PREVIEW_HEIGHT).expect("encode");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("valid base64");
        let mut reader = png::Decoder::new(bytes.as_slice())
            .read_info()
            .expect("valid png");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("frame");
        assert_eq!((info.width, info.height), (320, 180));
        assert_eq!(&buf[..info.buffer_size()], rgba.as_slice());
    }

    #[test]
    fn encoding_rejects_a_mismatched_buffer() {
        assert!(matches!(
            encode_png_base64(&[0; 10], 320, 180),
            Err(AppError::InvalidInput(_))
        ));
    }

    fn raw(source: &str, rgba_len: usize) -> RawFrame {
        RawFrame {
            source: source.into(),
            native_width: 1280,
            native_height: 720,
            rgba: vec![0; rgba_len],
            source_delay_ms: 5,
        }
    }

    #[tokio::test]
    async fn hub_drops_wrong_sized_frames_and_throttles_the_rest() {
        let hub = PreviewHub::new();
        let mut rx = hub.subscribe();
        hub.push(raw("a", 10));
        for _ in 0..50 {
            hub.push(raw("a", (PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize));
        }
        let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("a frame arrives")
            .expect("open");
        assert_eq!(
            (first.source.as_str(), first.width, first.height),
            ("a", 320, 180)
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(rx.try_recv().is_err(), "burst must collapse to one frame");
    }

    #[tokio::test]
    async fn flush_publishes_stats_and_snapshot_survives_resubscription() {
        let hub = PreviewHub::new();
        let (bus, _keep) = EventBus::new();
        let mut events = bus.subscribe();
        let mut rx = hub.subscribe();
        hub.push(raw("a", (PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize));
        rx.recv().await.expect("frame");
        hub.flush_stats(&bus, Instant::now() + Duration::from_secs(1));
        match events.try_recv().expect("stats event") {
            AppEvent::SourceStats {
                name,
                width,
                height,
                latency_ms,
                ..
            } => {
                assert_eq!((name.as_str(), width, height), ("a", 1280, 720));
                assert!(latency_ms >= 5);
            }
            other => panic!("unexpected {other:?}"),
        }
        let snap = hub.snapshot();
        assert_eq!(snap.frames.len(), 1);
        assert_eq!(snap.stats.len(), 1);
        hub.forget("a");
        assert!(hub.snapshot().frames.is_empty());
        assert!(hub.snapshot().stats.is_empty());
    }

    #[test]
    fn layout_validation_and_persistence() {
        assert!(validate_layout("2x3").is_ok());
        assert!(matches!(
            validate_layout("3x3"),
            Err(AppError::InvalidInput(_))
        ));
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("streamdeck.toml");
        assert_eq!(get_layout(&path).expect("default"), "2x2");
        set_layout(&path, "2x4").expect("set");
        assert_eq!(get_layout(&path).expect("get"), "2x4");
        assert!(set_layout(&path, "bogus").is_err());
        assert_eq!(get_layout(&path).expect("unchanged"), "2x4");
    }
}
