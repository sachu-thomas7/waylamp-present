// Shared mock-runtime harness for L2 Rust integration tests (real bus, real state, no webview).
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use streamdeck_lib::bus::EventBus;
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::display::{DisplayInfo, DisplayProvider};
use streamdeck_lib::error::AppError;
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::slide_state_machine::StateMachineHandle;
use tauri::test::{get_ipc_response, mock_context, noop_assets, MockRuntime};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::broadcast::{error::TryRecvError, Receiver};

// Tauri only treats this as the app's own (local) origin on the matching platform.
#[cfg(any(windows, target_os = "android"))]
const LOCAL_ORIGIN: &str = "http://tauri.localhost";
#[cfg(not(any(windows, target_os = "android")))]
const LOCAL_ORIGIN: &str = "tauri://localhost";

pub struct TestApp {
    pub app: App<MockRuntime>,
    pub window: WebviewWindow<MockRuntime>,
    pub bus: Arc<EventBus>,
    pub machine: StateMachineHandle,
    pub data_dir: tempfile::TempDir,
    pub displays: Arc<FakeDisplays>,
    pub pipeline_backend: streamdeck_lib::pipeline::SimulatedBackend,
    pub pipeline: streamdeck_lib::pipeline::PipelineHandle,
    pub ndi_discovery: streamdeck_lib::ndi::SimulatedDiscovery,
    pub ndi_provider: streamdeck_lib::ndi::NdiDiscoveryHandle,
    pub ndi_state: streamdeck_lib::ndi::NdiStateHandle,
    pub preview: streamdeck_lib::preview::PreviewHub,
    pub whip: Arc<streamdeck_lib::whip::WhipController>,
    pub whip_backend: streamdeck_lib::whip::SimulatedWhipBackend,
    pub whip_tokens: Arc<streamdeck_lib::whip::MemoryTokenStore>,
}

/// Scriptable monitor list standing in for the OS (mock `available_monitors` is unimplemented).
pub struct FakeDisplays {
    list: Mutex<Result<Vec<DisplayInfo>, String>>,
}

impl FakeDisplays {
    pub fn new(displays: Vec<DisplayInfo>) -> Self {
        Self {
            list: Mutex::new(Ok(displays)),
        }
    }

    pub fn set(&self, displays: Vec<DisplayInfo>) {
        *self.list.lock().expect("fake displays lock") = Ok(displays);
    }

    pub fn list_now(&self) -> Vec<DisplayInfo> {
        self.list().expect("fake displays are healthy")
    }

    pub fn fail(&self, reason: &str) {
        *self.list.lock().expect("fake displays lock") = Err(reason.into());
    }
}

impl DisplayProvider for FakeDisplays {
    fn list(&self) -> Result<Vec<DisplayInfo>, AppError> {
        self.list
            .lock()
            .expect("fake displays lock")
            .clone()
            .map_err(AppError::Internal)
    }
}

/// A 1920x1080 display named `name` at horizontal offset `x`; its id is `name@x,0`.
pub fn display(name: &str, x: i32, is_primary: bool) -> DisplayInfo {
    DisplayInfo {
        id: format!("{name}@{x},0"),
        name: name.into(),
        width: 1920,
        height: 1080,
        x,
        y: 0,
        is_primary,
    }
}

impl TestApp {
    pub fn new() -> Self {
        Self::with_config(StreamDeckConfig::default())
    }

    pub fn with_config(config: StreamDeckConfig) -> Self {
        Self::with_data_dir(tempfile::tempdir().expect("temp app-data dir"), config)
    }

    /// Boots an app on an existing data dir, simulating a restart over persisted files.
    pub fn with_data_dir(data_dir: tempfile::TempDir, config: StreamDeckConfig) -> Self {
        Self::build(data_dir, config, None)
    }

    /// Attached sources emit synthetic 320x180 frames at `source_fps` into the preview hub.
    pub fn with_preview_feeds(source_fps: u32) -> Self {
        let data_dir = tempfile::tempdir().expect("temp app-data dir");
        Self::build(data_dir, StreamDeckConfig::default(), Some(source_fps))
    }

    fn build(data_dir: tempfile::TempDir, config: StreamDeckConfig, feed_fps: Option<u32>) -> Self {
        let (bus, _rx) = EventBus::new();
        let bus = Arc::new(bus);

        let app = tauri::test::mock_builder()
            .invoke_handler(streamdeck_lib::state::mock_invoke_handler())
            .build(mock_context(noop_assets()))
            .expect("mock app builds");

        let machine = streamdeck_lib::state::manage_core(
            &app,
            Arc::clone(&bus),
            data_dir.path().to_path_buf(),
            data_dir.path().join("config").join("streamdeck.toml"),
            config.clone(),
        );
        let displays = Arc::new(FakeDisplays::new(vec![
            display("Primary", 0, true),
            display("Projector", 1920, false),
        ]));
        streamdeck_lib::state::manage_display(&app, &config, displays.clone());

        let preview = streamdeck_lib::state::manage_preview(&app);
        let pipeline_backend = match feed_fps {
            Some(fps) => {
                streamdeck_lib::pipeline::SimulatedBackend::with_preview(preview.clone(), fps)
            }
            None => streamdeck_lib::pipeline::SimulatedBackend::default(),
        };
        let pipeline = streamdeck_lib::state::manage_pipeline(
            &app,
            Box::new(pipeline_backend.clone()),
            Arc::clone(&bus),
        );

        let ndi_discovery = streamdeck_lib::ndi::SimulatedDiscovery::default();
        let ndi_provider: streamdeck_lib::ndi::NdiDiscoveryHandle = Arc::new(ndi_discovery.clone());
        let ndi_state = streamdeck_lib::state::manage_ndi(&app, Arc::clone(&ndi_provider));

        let whip_backend = match feed_fps {
            Some(fps) => {
                streamdeck_lib::whip::SimulatedWhipBackend::with_preview(preview.clone(), fps)
            }
            None => streamdeck_lib::whip::SimulatedWhipBackend::default(),
        };
        let whip_tokens = Arc::new(streamdeck_lib::whip::MemoryTokenStore::default());
        let whip = streamdeck_lib::state::manage_whip(
            &app,
            Arc::new(whip_backend.clone()),
            Arc::clone(&bus),
            data_dir.path().join("config").join("streamdeck.toml"),
            Arc::new(streamdeck_lib::whip::FixedLocalIp(vec!["192.168.1.20"
                .parse()
                .expect("ip")])),
            whip_tokens.clone(),
        );

        let window = WebviewWindowBuilder::new(&app, "controller", WebviewUrl::default())
            .build()
            .expect("mock controller window");

        Self {
            app,
            window,
            bus,
            machine,
            data_dir,
            displays,
            pipeline_backend,
            pipeline,
            ndi_discovery,
            ndi_provider,
            ndi_state,
            preview,
            whip,
            whip_backend,
            whip_tokens,
        }
    }

    /// One NDI poll step, as the 2s watch task would run it.
    pub async fn poll_ndi(&self) {
        streamdeck_lib::ndi::reconcile_sources(
            &self.ndi_provider,
            &self.pipeline,
            &self.bus,
            &self.ndi_state,
        )
        .await;
    }

    /// The token currently held in the in-memory keychain.
    pub fn whip_token(&self) -> Option<String> {
        use streamdeck_lib::whip::TokenStore;
        self.whip_tokens.get().expect("token store")
    }

    /// Points the WHIP endpoint at loopback with an OS-assigned port, as a test must never bind 0.0.0.0:8088.
    pub fn use_loopback_whip(&self) {
        let path = self.config_path();
        let mut config =
            streamdeck_lib::config_loader::ConfigLoader::load_or_default(&path).expect("config");
        config.whip.bind_address = "127.0.0.1".into();
        config.whip.port = 0;
        streamdeck_lib::config_loader::ConfigLoader::save(&path, &config).expect("save config");
    }

    /// The `streamdeck.toml` the app under test reads/writes (inside the temp data dir).
    pub fn config_path(&self) -> std::path::PathBuf {
        self.data_dir.path().join("config").join("streamdeck.toml")
    }

    /// Subscribe before acting so no event published by the command is missed.
    pub fn collect_events(&self) -> EventCollector {
        EventCollector::subscribe(&self.bus)
    }

    /// Calls a command through the real IPC dispatch path and decodes the JSON result.
    pub fn invoke<T: DeserializeOwned>(
        &self,
        cmd: &str,
        args: serde_json::Value,
    ) -> Result<T, serde_json::Value> {
        get_ipc_response(
            &self.window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: LOCAL_ORIGIN.parse().expect("valid url"),
                body: tauri::ipc::InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
        .map(|body| {
            body.deserialize::<T>()
                .expect("command response matches the expected type")
        })
    }
}

pub struct EventCollector {
    rx: Receiver<AppEvent>,
}

impl EventCollector {
    pub fn subscribe(bus: &EventBus) -> Self {
        Self {
            rx: bus.subscribe(),
        }
    }

    /// Returns every event already published since subscribing (non-blocking).
    pub fn drain(&mut self) -> Vec<AppEvent> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Lagged(_)) => continue,
                Err(TryRecvError::Empty | TryRecvError::Closed) => return out,
            }
        }
    }

    /// Collects events until `done(&events)` holds or `deadline` passes; returns what was seen.
    pub async fn collect_until(
        &mut self,
        deadline: Duration,
        done: impl Fn(&[AppEvent]) -> bool,
    ) -> Vec<AppEvent> {
        let mut seen = Vec::new();
        let _ = tokio::time::timeout(deadline, async {
            while !done(&seen) {
                match self.rx.recv().await {
                    Ok(e) => seen.push(e),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        })
        .await;
        seen
    }
}
