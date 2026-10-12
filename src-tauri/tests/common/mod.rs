// Shared mock-runtime harness for L2 Rust integration tests (real bus, real state, no webview).
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use streamdeck_lib::bus::EventBus;
use streamdeck_lib::config::StreamDeckConfig;
use streamdeck_lib::events::AppEvent;
use streamdeck_lib::slide_state_machine::StateMachineHandle;
use tauri::test::{get_ipc_response, mock_context, noop_assets, MockRuntime};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::broadcast::{error::TryRecvError, Receiver};

pub struct TestApp {
    pub app: App<MockRuntime>,
    pub window: WebviewWindow<MockRuntime>,
    pub bus: Arc<EventBus>,
    pub machine: StateMachineHandle,
    pub data_dir: tempfile::TempDir,
}

impl TestApp {
    pub fn new() -> Self {
        Self::with_config(StreamDeckConfig::default())
    }

    pub fn with_config(config: StreamDeckConfig) -> Self {
        let data_dir = tempfile::tempdir().expect("temp app-data dir");
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
            config,
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
        }
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
                url: "http://tauri.localhost".parse().expect("valid url"),
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
