//! `StreamController`: owns the stream lifecycle, per-destination state machines and reconnects on top of a
//! `StreamBackend` (the engine). Keys come from a `KeyStore` (OS keychain), never from TOML.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::bus::EventBus;
use crate::config::StreamDeckConfig;
use crate::config_loader::ConfigLoader;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::stream::{
    self, AudioSource, Backoff, Destination, DestinationState, DestinationStatus, EncoderChoice,
    StreamSettings, StreamState, StreamStatus, VideoSource,
};

pub const KEYCHAIN_SERVICE: &str = "StreamDeck";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ── Key storage ─────────────────────────────────────────────────────────────

pub trait KeyStore: Send + Sync + 'static {
    fn get(&self, id: &str) -> Result<Option<String>, AppError>;
    fn set(&self, id: &str, key: Option<&str>) -> Result<(), AppError>;
}

pub struct KeyringKeyStore;

impl KeyringKeyStore {
    fn entry(id: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, &format!("stream-key-{id}"))
            .map_err(|e| AppError::Keychain(e.to_string()))
    }
}

impl KeyStore for KeyringKeyStore {
    fn get(&self, id: &str) -> Result<Option<String>, AppError> {
        match Self::entry(id)?.get_password() {
            Ok(k) => Ok(Some(k)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Keychain(e.to_string())),
        }
    }

    fn set(&self, id: &str, key: Option<&str>) -> Result<(), AppError> {
        let entry = Self::entry(id)?;
        match key {
            Some(k) => entry
                .set_password(k)
                .map_err(|e| AppError::Keychain(e.to_string())),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(AppError::Keychain(e.to_string())),
            },
        }
    }
}

#[derive(Default)]
pub struct MemoryKeyStore {
    keys: Mutex<HashMap<String, String>>,
    fail: Mutex<Option<String>>,
}

impl MemoryKeyStore {
    pub fn failing(&self, reason: Option<&str>) {
        *lock(&self.fail) = reason.map(String::from);
    }
}

impl KeyStore for MemoryKeyStore {
    fn get(&self, id: &str) -> Result<Option<String>, AppError> {
        if let Some(r) = lock(&self.fail).clone() {
            return Err(AppError::Keychain(r));
        }
        Ok(lock(&self.keys).get(id).cloned())
    }

    fn set(&self, id: &str, key: Option<&str>) -> Result<(), AppError> {
        if let Some(r) = lock(&self.fail).clone() {
            return Err(AppError::Keychain(r));
        }
        match key {
            Some(k) => lock(&self.keys).insert(id.into(), k.into()),
            None => lock(&self.keys).remove(id),
        };
        Ok(())
    }
}

// ── Engine seam ─────────────────────────────────────────────────────────────

pub struct StartPlan {
    pub settings: StreamSettings,
    pub video: VideoSource,
    pub audio: AudioSource,
    pub allow_gpl_encoder: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackendEvent {
    DestinationUp(String),
    DestinationDown {
        id: String,
        reason: String,
    },
    Stats {
        id: String,
        bitrate_kbps: u32,
        dropped_frames: u64,
    },
}

/// The media engine. `url` already contains the stream key: implementations must never log it.
pub trait StreamBackend: Send + 'static {
    /// Builds and starts the encode pipeline (no destinations yet) and reports the encoder used.
    fn start(&mut self, plan: &StartPlan) -> Result<EncoderChoice, AppError>;
    fn add_destination(&mut self, id: &str, url: &str) -> Result<(), AppError>;
    /// Idempotent.
    fn remove_destination(&mut self, id: &str) -> Result<(), AppError>;
    /// Graceful EOS then NULL; idempotent.
    fn stop(&mut self) -> Result<(), AppError>;
    fn poll(&mut self) -> Vec<BackendEvent>;
}

pub struct UnavailableStreamBackend(pub String);

impl StreamBackend for UnavailableStreamBackend {
    fn start(&mut self, _: &StartPlan) -> Result<EncoderChoice, AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn add_destination(&mut self, _: &str, _: &str) -> Result<(), AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn remove_destination(&mut self, _: &str) -> Result<(), AppError> {
        Ok(())
    }
    fn stop(&mut self) -> Result<(), AppError> {
        Ok(())
    }
    fn poll(&mut self) -> Vec<BackendEvent> {
        Vec::new()
    }
}

/// Records calls and replays scripted events; keys in `add_destination` URLs are never stored in `calls`.
#[derive(Clone, Default)]
pub struct SimulatedStreamBackend {
    inner: Arc<Mutex<SimInner>>,
}

#[derive(Default)]
struct SimInner {
    calls: Vec<String>,
    events: Vec<BackendEvent>,
    fail_start: Option<String>,
    fail_add: HashMap<String, String>,
    auto_up: bool,
    urls: HashMap<String, String>,
}

impl SimulatedStreamBackend {
    pub fn new() -> Self {
        let s = Self::default();
        lock(&s.inner).auto_up = true;
        s
    }
    pub fn calls(&self) -> Vec<String> {
        lock(&self.inner).calls.clone()
    }
    pub fn push_event(&self, e: BackendEvent) {
        lock(&self.inner).events.push(e);
    }
    pub fn fail_start(&self, reason: Option<&str>) {
        lock(&self.inner).fail_start = reason.map(String::from);
    }
    pub fn fail_add(&self, id: &str, reason: Option<&str>) {
        let mut g = lock(&self.inner);
        match reason {
            Some(r) => g.fail_add.insert(id.into(), r.into()),
            None => g.fail_add.remove(id),
        };
    }
    /// With false, destinations stay `Connecting` until the test pushes `DestinationUp`.
    pub fn auto_up(&self, on: bool) {
        lock(&self.inner).auto_up = on;
    }
    /// The URL (with key) last given for `id`: lets tests prove the key reached the engine.
    pub fn url_for(&self, id: &str) -> Option<String> {
        lock(&self.inner).urls.get(id).cloned()
    }
}

impl StreamBackend for SimulatedStreamBackend {
    fn start(&mut self, plan: &StartPlan) -> Result<EncoderChoice, AppError> {
        let mut g = lock(&self.inner);
        g.calls.push(format!(
            "start:{}x{}@{}:{}kbps",
            plan.settings.width, plan.settings.height, plan.settings.fps, plan.settings.video_kbps
        ));
        if let Some(r) = g.fail_start.clone() {
            return Err(AppError::Internal(r));
        }
        Ok(EncoderChoice {
            element: "x264enc",
            hardware: false,
        })
    }
    fn add_destination(&mut self, id: &str, url: &str) -> Result<(), AppError> {
        let mut g = lock(&self.inner);
        g.calls.push(format!("add:{id}"));
        if let Some(r) = g.fail_add.get(id).cloned() {
            return Err(AppError::Internal(r));
        }
        g.urls.insert(id.into(), url.into());
        if g.auto_up {
            g.events.push(BackendEvent::DestinationUp(id.into()));
        }
        Ok(())
    }
    fn remove_destination(&mut self, id: &str) -> Result<(), AppError> {
        let mut g = lock(&self.inner);
        g.calls.push(format!("remove:{id}"));
        g.urls.remove(id);
        Ok(())
    }
    fn stop(&mut self) -> Result<(), AppError> {
        let mut g = lock(&self.inner);
        g.calls.push("stop".into());
        g.urls.clear();
        g.events.clear();
        Ok(())
    }
    fn poll(&mut self) -> Vec<BackendEvent> {
        std::mem::take(&mut lock(&self.inner).events)
    }
}

// ── Controller ──────────────────────────────────────────────────────────────

struct Live {
    started: Instant,
    started_unix_ms: u64,
    encoder: EncoderChoice,
}

struct DestRuntime {
    state: DestinationState,
    backoff: Backoff,
    retry_at: Option<Instant>,
    bitrate_kbps: u32,
    dropped_frames: u64,
}

impl DestRuntime {
    fn new() -> Self {
        Self {
            state: DestinationState::Connecting,
            backoff: Backoff::default(),
            retry_at: None,
            bitrate_kbps: 0,
            dropped_frames: 0,
        }
    }
}

struct Inner {
    backend: Box<dyn StreamBackend>,
    live: Option<Live>,
    runtime: HashMap<String, DestRuntime>,
    state: StreamState,
}

/// Where the program video and the audio come from; the app wires the compositor's `program` channel here.
pub struct Sources {
    pub video: VideoSource,
    pub audio: AudioSource,
}

pub struct StreamController {
    inner: Mutex<Inner>,
    bus: Arc<EventBus>,
    config_path: PathBuf,
    keys: Arc<dyn KeyStore>,
    sources: Sources,
    allow_gpl_encoder: bool,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DestinationConfigView {
    #[serde(flatten)]
    pub destination: Destination,
    pub key_set: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamConfigView {
    pub settings: StreamSettings,
    pub destinations: Vec<DestinationConfigView>,
}

impl StreamController {
    pub fn new(
        backend: Box<dyn StreamBackend>,
        bus: Arc<EventBus>,
        config_path: PathBuf,
        keys: Arc<dyn KeyStore>,
        sources: Sources,
        allow_gpl_encoder: bool,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                backend,
                live: None,
                runtime: HashMap::new(),
                state: StreamState::Stopped,
            }),
            bus,
            config_path,
            keys,
            sources,
            allow_gpl_encoder,
        }
    }

    fn config(&self) -> Result<StreamDeckConfig, AppError> {
        ConfigLoader::load_or_default(&self.config_path)
    }

    fn save_config(
        &self,
        f: impl FnOnce(&mut StreamDeckConfig) -> Result<(), AppError>,
    ) -> Result<(), AppError> {
        let mut cfg = self.config()?;
        f(&mut cfg)?;
        ConfigLoader::save(&self.config_path, &cfg)
    }

    pub fn config_view(&self) -> Result<StreamConfigView, AppError> {
        let cfg = self.config()?;
        let destinations = cfg
            .stream
            .destinations
            .iter()
            .map(|d| {
                Ok(DestinationConfigView {
                    destination: d.clone(),
                    key_set: self.keys.get(&d.id)?.is_some(),
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        Ok(StreamConfigView {
            settings: cfg.stream.settings,
            destinations,
        })
    }

    pub fn status(&self) -> Result<StreamStatus, AppError> {
        let cfg = self.config()?;
        let g = lock(&self.inner);
        Ok(Self::status_of(&g, &cfg.stream.destinations))
    }

    fn status_of(g: &Inner, destinations: &[Destination]) -> StreamStatus {
        let live = g.live.as_ref();
        StreamStatus {
            state: g.state.clone(),
            encoder: live.map(|l| l.encoder.element.to_string()),
            hardware: live.is_some_and(|l| l.encoder.hardware),
            started_unix_ms: live.map(|l| l.started_unix_ms),
            destinations: destinations
                .iter()
                .map(|d| {
                    let rt = g.runtime.get(&d.id);
                    DestinationStatus {
                        id: d.id.clone(),
                        name: d.name.clone(),
                        platform: d.platform,
                        state: rt.map_or(DestinationState::Idle, |r| r.state.clone()),
                        bitrate_kbps: rt.map_or(0, |r| r.bitrate_kbps),
                        dropped_frames: rt.map_or(0, |r| r.dropped_frames),
                    }
                })
                .collect(),
        }
    }

    // ── configuration ──

    /// Adds or updates a destination. A key, when given, goes to the keychain; `None` keeps the existing key.
    pub fn save_destination(
        &self,
        dest: Destination,
        key: Option<String>,
    ) -> Result<StreamConfigView, AppError> {
        stream::validate_destination(&dest)?;
        if let Some(k) = key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
            stream::validate_stream_key(k)?;
            self.keys.set(&dest.id, Some(k))?;
        }
        self.save_config(|cfg| {
            let list = &mut cfg.stream.destinations;
            match list.iter_mut().find(|d| d.id == dest.id) {
                Some(existing) => *existing = dest.clone(),
                None => list.push(dest.clone()),
            }
            stream::validate_destinations(list)
        })?;
        if self.is_live() {
            self.apply_live_enabled(&dest.id, dest.enabled)?;
        }
        self.config_view()
    }

    pub fn remove_destination(&self, id: &str) -> Result<StreamConfigView, AppError> {
        stream::validate_destination_id(id)?;
        if self.is_live() {
            self.apply_live_enabled(id, false)?;
        }
        self.save_config(|cfg| {
            cfg.stream.destinations.retain(|d| d.id != id);
            Ok(())
        })?;
        self.keys.set(id, None)?;
        lock(&self.inner).runtime.remove(id);
        self.config_view()
    }

    pub fn set_settings(&self, settings: StreamSettings) -> Result<StreamConfigView, AppError> {
        if self.is_live() {
            return Err(AppError::InvalidInput(
                "stream settings cannot change while live".into(),
            ));
        }
        settings.validate()?;
        self.save_config(|cfg| {
            cfg.stream.settings = settings;
            Ok(())
        })?;
        self.config_view()
    }

    fn is_live(&self) -> bool {
        lock(&self.inner).live.is_some()
    }

    // ── lifecycle ──

    pub fn start(&self) -> Result<StreamStatus, AppError> {
        let cfg = self.config()?;
        cfg.stream.settings.validate()?;
        let enabled: Vec<&Destination> = cfg
            .stream
            .destinations
            .iter()
            .filter(|d| d.enabled)
            .collect();
        if enabled.is_empty() {
            return Err(AppError::InvalidInput(
                "add and enable at least one destination first".into(),
            ));
        }
        let mut targets = Vec::new();
        for d in &enabled {
            let key = self.keys.get(&d.id)?.ok_or_else(|| {
                AppError::InvalidInput(format!("destination '{}' has no stream key", d.name))
            })?;
            targets.push(((*d).clone(), stream::full_url(&d.ingest_url, &key), key));
        }

        let mut g = lock(&self.inner);
        if g.live.is_some() || g.state != StreamState::Stopped {
            return Err(AppError::InvalidInput(
                "the stream is already running".into(),
            ));
        }
        g.state = StreamState::Starting;
        self.bus.publish(AppEvent::StreamStateChanged {
            state: StreamState::Starting,
        });
        let plan = StartPlan {
            settings: cfg.stream.settings.clone(),
            video: self.sources.video.clone(),
            audio: self.sources.audio.clone(),
            allow_gpl_encoder: self.allow_gpl_encoder,
        };
        let secrets: Vec<&str> = targets.iter().map(|(_, _, k)| k.as_str()).collect();
        let encoder = match g.backend.start(&plan) {
            Ok(e) => e,
            Err(e) => {
                g.state = StreamState::Stopped;
                self.bus.publish(AppEvent::StreamStateChanged {
                    state: StreamState::Stopped,
                });
                return Err(AppError::Internal(stream::redact(&e.to_string(), &secrets)));
            }
        };
        g.live = Some(Live {
            started: Instant::now(),
            started_unix_ms: now_unix_ms(),
            encoder,
        });
        g.runtime.clear();
        for (d, url, key) in &targets {
            g.runtime.insert(d.id.clone(), DestRuntime::new());
            self.bus.publish(AppEvent::StreamDestinationChanged {
                id: d.id.clone(),
                state: DestinationState::Connecting,
            });
            if let Err(e) = g.backend.add_destination(&d.id, url) {
                let msg = stream::redact(&e.to_string(), &[key.as_str()]);
                self.set_dest_state(&mut g, &d.id, DestinationState::Failed(msg));
            }
        }
        g.state = StreamState::Live;
        self.bus.publish(AppEvent::StreamStateChanged {
            state: StreamState::Live,
        });
        Ok(Self::status_of(&g, &cfg.stream.destinations))
    }

    pub fn stop(&self) -> Result<StreamStatus, AppError> {
        let cfg = self.config()?;
        let mut g = lock(&self.inner);
        if g.live.is_none() {
            return Ok(Self::status_of(&g, &cfg.stream.destinations));
        }
        g.state = StreamState::Stopping;
        self.bus.publish(AppEvent::StreamStateChanged {
            state: StreamState::Stopping,
        });
        let result = g.backend.stop();
        let ids: Vec<String> = g.runtime.keys().cloned().collect();
        g.runtime.clear();
        g.live = None;
        g.state = StreamState::Stopped;
        for id in ids {
            self.bus.publish(AppEvent::StreamDestinationChanged {
                id,
                state: DestinationState::Idle,
            });
        }
        self.bus.publish(AppEvent::StreamStateChanged {
            state: StreamState::Stopped,
        });
        result?;
        Ok(Self::status_of(&g, &cfg.stream.destinations))
    }

    /// Mutes (removes the branch of) or unmutes one destination without touching the others.
    pub fn set_destination_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<StreamConfigView, AppError> {
        stream::validate_destination_id(id)?;
        self.save_config(|cfg| {
            let d = cfg
                .stream
                .destinations
                .iter_mut()
                .find(|d| d.id == id)
                .ok_or_else(|| AppError::NotFound(format!("unknown destination: {id}")))?;
            d.enabled = enabled;
            Ok(())
        })?;
        if self.is_live() {
            self.apply_live_enabled(id, enabled)?;
        }
        self.config_view()
    }

    fn apply_live_enabled(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        let cfg = self.config()?;
        let mut g = lock(&self.inner);
        if g.live.is_none() {
            return Ok(());
        }
        let running = g.runtime.contains_key(id);
        if enabled && !running {
            let Some(d) = cfg.stream.destinations.iter().find(|d| d.id == id) else {
                return Ok(());
            };
            let key = self.keys.get(id)?.ok_or_else(|| {
                AppError::InvalidInput(format!("destination '{}' has no stream key", d.name))
            })?;
            g.runtime.insert(id.to_string(), DestRuntime::new());
            self.bus.publish(AppEvent::StreamDestinationChanged {
                id: id.to_string(),
                state: DestinationState::Connecting,
            });
            if let Err(e) = g
                .backend
                .add_destination(id, &stream::full_url(&d.ingest_url, &key))
            {
                let msg = stream::redact(&e.to_string(), &[key.as_str()]);
                self.set_dest_state(&mut g, id, DestinationState::Failed(msg));
            }
        } else if !enabled && running {
            let _ = g.backend.remove_destination(id);
            g.runtime.remove(id);
            self.bus.publish(AppEvent::StreamDestinationChanged {
                id: id.to_string(),
                state: DestinationState::Idle,
            });
        }
        Ok(())
    }

    fn set_dest_state(&self, g: &mut Inner, id: &str, state: DestinationState) {
        if let Some(rt) = g.runtime.get_mut(id) {
            if rt.state != state {
                rt.state = state.clone();
                self.bus.publish(AppEvent::StreamDestinationChanged {
                    id: id.to_string(),
                    state,
                });
            }
        }
    }

    /// Drains engine events, schedules and performs reconnects. Called about once a second.
    pub fn tick(&self, now: Instant) -> Result<(), AppError> {
        let cfg = self.config()?;
        let mut g = lock(&self.inner);
        if g.live.is_none() {
            return Ok(());
        }
        for ev in g.backend.poll() {
            match ev {
                BackendEvent::DestinationUp(id) => {
                    if let Some(rt) = g.runtime.get_mut(&id) {
                        rt.backoff.reset();
                        rt.retry_at = None;
                    }
                    self.set_dest_state(&mut g, &id, DestinationState::Live);
                }
                BackendEvent::DestinationDown { id, reason } => {
                    let _ = reason; // engine reasons can echo the URL, so they are not forwarded
                    if let Some(rt) = g.runtime.get_mut(&id) {
                        let delay = rt.backoff.next_delay();
                        rt.retry_at = Some(now + delay);
                        let state = DestinationState::Reconnecting {
                            attempt: rt.backoff.attempts(),
                            retry_in_secs: delay.as_secs(),
                        };
                        let _ = g.backend.remove_destination(&id);
                        self.set_dest_state(&mut g, &id, state);
                    }
                }
                BackendEvent::Stats {
                    id,
                    bitrate_kbps,
                    dropped_frames,
                } => {
                    if let Some(rt) = g.runtime.get_mut(&id) {
                        rt.bitrate_kbps = bitrate_kbps;
                        rt.dropped_frames = dropped_frames;
                    }
                    self.bus.publish(AppEvent::StreamHealth {
                        id,
                        bitrate_kbps,
                        dropped_frames,
                    });
                }
            }
        }
        let due: Vec<String> = g
            .runtime
            .iter()
            .filter(|(_, rt)| rt.retry_at.is_some_and(|t| t <= now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            let Some(d) = cfg.stream.destinations.iter().find(|d| d.id == id) else {
                continue;
            };
            let key = self.keys.get(&id)?.unwrap_or_default();
            if let Some(rt) = g.runtime.get_mut(&id) {
                rt.retry_at = None;
            }
            if g.backend
                .add_destination(&id, &stream::full_url(&d.ingest_url, &key))
                .is_err()
            {
                if let Some(rt) = g.runtime.get_mut(&id) {
                    let delay = rt.backoff.next_delay();
                    rt.retry_at = Some(now + delay);
                    let state = DestinationState::Reconnecting {
                        attempt: rt.backoff.attempts(),
                        retry_in_secs: delay.as_secs(),
                    };
                    self.set_dest_state(&mut g, &id, state);
                }
            }
        }
        Ok(())
    }

    /// Final teardown at app exit.
    pub fn shutdown(&self) {
        let _ = self.stop();
    }

    pub fn elapsed(&self) -> Option<Duration> {
        lock(&self.inner).live.as_ref().map(|l| l.started.elapsed())
    }
}

/// Polls once a second so reconnects and health run without any UI attached.
pub fn spawn_tick_task(controller: Arc<StreamController>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let c = Arc::clone(&controller);
            let _ = tokio::task::spawn_blocking(move || c.tick(Instant::now())).await;
        }
    })
}
