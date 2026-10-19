use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::RwLock;

use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::pipeline::PipelineHandle;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NdiSource {
    pub name: String,
    pub url: Option<String>,
    pub advertises_new_format: bool,
}

impl NdiSource {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            url: None,
            advertises_new_format: false,
        }
    }

    pub fn with_new_format(mut self) -> Self {
        self.advertises_new_format = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum StreamFormat {
    NewFormat,
    Standard,
}

/// NDI 6.3 New Format Stream only when the source advertises it and the runtime can receive it.
pub fn select_stream_format(source: &NdiSource, runtime_supports_new_format: bool) -> StreamFormat {
    if source.advertises_new_format && runtime_supports_new_format {
        StreamFormat::NewFormat
    } else {
        StreamFormat::Standard
    }
}

/// The per-source GStreamer segment; the name is quoted so spaces and quotes survive parsing.
pub fn ndi_source_description(name: &str) -> String {
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "ndisrc ndi-name=\"{escaped}\" ! queue ! videoconvert ! videoscale name=scale add-borders=true ! video/x-raw,format=RGBA,width=320,height=180 ! appsink name=sink max-buffers=2 drop=true sync=false"
    )
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SourceDiff {
    pub discovered: Vec<NdiSource>,
    pub lost: Vec<String>,
}

/// Sorted by name with duplicate names dropped; a name is a source's identity.
pub fn normalize(mut sources: Vec<NdiSource>) -> Vec<NdiSource> {
    sources.sort_by(|a, b| a.name.cmp(&b.name));
    sources.dedup_by(|a, b| a.name == b.name);
    sources
}

pub fn diff_sources(previous: &[NdiSource], current: &[NdiSource]) -> SourceDiff {
    SourceDiff {
        discovered: current
            .iter()
            .filter(|c| !previous.iter().any(|p| p.name == c.name))
            .cloned()
            .collect(),
        lost: previous
            .iter()
            .filter(|p| !current.iter().any(|c| c.name == p.name))
            .map(|p| p.name.clone())
            .collect(),
    }
}

/// The NDI network scan; `list` may block, so it is only called off the async threads.
pub trait NdiDiscovery: Send + Sync + 'static {
    fn list(&self) -> Result<Vec<NdiSource>, AppError>;

    /// Whether the runtime can receive NDI 6.3 New Format Streams.
    fn supports_new_format(&self) -> bool {
        false
    }

    /// Why discovery can never work in this build, if so.
    fn unavailable_reason(&self) -> Option<String> {
        None
    }
}

pub type NdiDiscoveryHandle = Arc<dyn NdiDiscovery>;

/// Used when the `ndi` feature is off or the NDI runtime failed to initialise.
pub struct UnavailableDiscovery(pub String);

impl NdiDiscovery for UnavailableDiscovery {
    fn list(&self) -> Result<Vec<NdiSource>, AppError> {
        Ok(Vec::new())
    }
    fn unavailable_reason(&self) -> Option<String> {
        Some(self.0.clone())
    }
}

/// Scriptable network for tests and the mock harness.
#[derive(Clone, Default)]
pub struct SimulatedDiscovery {
    sources: Arc<Mutex<Vec<NdiSource>>>,
    failure: Arc<Mutex<Option<String>>>,
    new_format_runtime: Arc<AtomicBool>,
}

impl SimulatedDiscovery {
    pub fn set(&self, sources: Vec<NdiSource>) {
        if let Ok(mut s) = self.sources.lock() {
            *s = sources;
        }
    }

    pub fn appear(&self, source: NdiSource) {
        if let Ok(mut s) = self.sources.lock() {
            s.retain(|x| x.name != source.name);
            s.push(source);
        }
    }

    pub fn disappear(&self, name: &str) {
        if let Ok(mut s) = self.sources.lock() {
            s.retain(|x| x.name != name);
        }
    }

    pub fn fail_with(&self, reason: Option<&str>) {
        if let Ok(mut f) = self.failure.lock() {
            *f = reason.map(String::from);
        }
    }

    pub fn set_new_format_runtime(&self, supported: bool) {
        self.new_format_runtime.store(supported, Ordering::Release);
    }
}

impl NdiDiscovery for SimulatedDiscovery {
    fn list(&self) -> Result<Vec<NdiSource>, AppError> {
        if let Some(reason) = self.failure.lock().ok().and_then(|f| f.clone()) {
            return Err(AppError::Internal(reason));
        }
        Ok(self.sources.lock().map(|s| s.clone()).unwrap_or_default())
    }
    fn supports_new_format(&self) -> bool {
        self.new_format_runtime.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AddedSource {
    pub name: String,
    pub format: StreamFormat,
    pub lost: bool,
}

#[derive(Debug, Default)]
pub struct NdiState {
    pub discovered: Vec<NdiSource>,
    pub added: Vec<AddedSource>,
}

pub type NdiStateHandle = Arc<RwLock<NdiState>>;

pub fn new_ndi_handle() -> NdiStateHandle {
    Arc::new(RwLock::new(NdiState::default()))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NdiSnapshot {
    pub discovered: Vec<NdiSource>,
    pub added: Vec<AddedSource>,
    pub unavailable: Option<String>,
}

pub async fn snapshot(provider: &NdiDiscoveryHandle, state: &NdiStateHandle) -> NdiSnapshot {
    let s = state.read().await;
    NdiSnapshot {
        discovered: s.discovered.clone(),
        added: s.added.clone(),
        unavailable: provider.unavailable_reason(),
    }
}

/// Adds a discovered source to the running media engine without touching the test pipeline.
pub async fn add_source(
    name: &str,
    provider: &NdiDiscoveryHandle,
    pipeline: &PipelineHandle,
    bus: &EventBus,
    state: &NdiStateHandle,
) -> Result<(), AppError> {
    let mut s = state.write().await;
    let source = s
        .discovered
        .iter()
        .find(|x| x.name == name)
        .ok_or_else(|| AppError::NotFound(format!("NDI source not found: {name}")))?
        .clone();
    if s.added.iter().any(|a| a.name == name) {
        return Err(AppError::InvalidInput(format!(
            "NDI source already added: {name}"
        )));
    }
    let format = select_stream_format(&source, provider.supports_new_format());
    pipeline.add_ndi_source(name, format).await?;
    s.added.push(AddedSource {
        name: name.into(),
        format,
        lost: false,
    });
    bus.publish(AppEvent::SourceAdded {
        name: name.into(),
        format,
    });
    Ok(())
}

pub async fn remove_source(
    name: &str,
    pipeline: &PipelineHandle,
    bus: &EventBus,
    state: &NdiStateHandle,
) -> Result<(), AppError> {
    let mut s = state.write().await;
    let pos = s
        .added
        .iter()
        .position(|a| a.name == name)
        .ok_or_else(|| AppError::NotFound(format!("NDI source not added: {name}")))?;
    pipeline.remove_ndi_source(name).await?;
    s.added.remove(pos);
    bus.publish(AppEvent::SourceRemoved { name: name.into() });
    Ok(())
}

/// Poll the network every 2s via `reconcile_sources`.
pub fn spawn_watch_task(
    provider: NdiDiscoveryHandle,
    pipeline: PipelineHandle,
    bus: Arc<EventBus>,
    state: NdiStateHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    // Called from .setup(), outside any Tokio runtime — tokio::spawn would panic here.
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            reconcile_sources(&provider, &pipeline, &bus, &state).await;
        }
    })
}

/// One poll step: diffs the network against known sources, publishes the changes, tears a lost
/// added source out of the engine and re-attaches it when it returns. A failed scan changes nothing.
pub async fn reconcile_sources(
    provider: &NdiDiscoveryHandle,
    pipeline: &PipelineHandle,
    bus: &EventBus,
    state: &NdiStateHandle,
) {
    let scan = {
        let p = Arc::clone(provider);
        tokio::task::spawn_blocking(move || p.list()).await
    };
    let current = match scan {
        Ok(Ok(list)) => normalize(list),
        Ok(Err(e)) => {
            eprintln!("NDI discovery failed: {e}");
            return;
        }
        Err(_) => return,
    };

    let mut s = state.write().await;
    let diff = diff_sources(&s.discovered, &current);

    for added in s.added.iter_mut() {
        let present = current.iter().any(|c| c.name == added.name);
        if !present && !added.lost {
            if let Err(e) = pipeline.remove_ndi_source(&added.name).await {
                eprintln!("NDI source teardown failed for {}: {e}", added.name);
            }
            added.lost = true;
        } else if present && added.lost {
            match pipeline.add_ndi_source(&added.name, added.format).await {
                Ok(()) => added.lost = false,
                Err(e) => bus.publish(AppEvent::PipelineError {
                    element: added.name.clone(),
                    message: format!("NDI source could not be re-attached: {e}"),
                }),
            }
        }
    }

    if diff == SourceDiff::default() {
        return;
    }
    s.discovered = current.clone();
    for name in diff.lost {
        bus.publish(AppEvent::SourceLost { name });
    }
    for source in diff.discovered {
        bus.publish(AppEvent::SourceDiscovered { source });
    }
    bus.publish(AppEvent::SourceListChanged { sources: current });
}

/// NDI discovery through gst-plugin-ndi's device provider; needs the NDI runtime installed.
#[cfg(feature = "ndi")]
pub mod gst_discovery {
    use gstreamer as gst;
    use gstreamer::prelude::*;

    use super::*;

    pub struct GstNdiDiscovery {
        monitor: gst::DeviceMonitor,
    }

    impl GstNdiDiscovery {
        pub fn new() -> Result<Self, AppError> {
            gst::init().map_err(|e| AppError::Internal(format!("GStreamer init failed: {e}")))?;
            let monitor = gst::DeviceMonitor::new();
            monitor.add_filter(Some("Source/Network"), None);
            // A started monitor keeps the NDI finder running so each poll is just a read.
            monitor
                .start()
                .map_err(|e| AppError::Internal(format!("NDI device monitor failed: {e}")))?;
            Ok(Self { monitor })
        }
    }

    impl NdiDiscovery for GstNdiDiscovery {
        fn list(&self) -> Result<Vec<NdiSource>, AppError> {
            Ok(self
                .monitor
                .devices()
                .into_iter()
                .map(|d| {
                    let props = d.properties();
                    let field = |k: &str| {
                        props
                            .as_ref()
                            .and_then(|p| p.get::<String>(k).ok())
                            .filter(|v| !v.is_empty())
                    };
                    NdiSource {
                        name: field("ndi-name").unwrap_or_else(|| d.display_name().to_string()),
                        url: field("url-address"),
                        advertises_new_format: false,
                    }
                })
                .collect())
        }

        fn supports_new_format(&self) -> bool {
            false
        }
    }

    impl Drop for GstNdiDiscovery {
        fn drop(&mut self) {
            self.monitor.stop();
        }
    }
}

#[cfg(feature = "ndi")]
pub fn production_discovery() -> NdiDiscoveryHandle {
    match gst_discovery::GstNdiDiscovery::new() {
        Ok(d) => Arc::new(d),
        Err(e) => {
            eprintln!("{e}");
            Arc::new(UnavailableDiscovery(e.to_string()))
        }
    }
}

#[cfg(not(feature = "ndi"))]
pub fn production_discovery() -> NdiDiscoveryHandle {
    Arc::new(UnavailableDiscovery(
        "NDI support not compiled in (build with --features ndi)".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{PipelineService, SimulatedBackend};

    fn n(name: &str) -> NdiSource {
        NdiSource::new(name)
    }

    #[test]
    fn new_format_needs_both_the_source_and_the_runtime() {
        let plain = n("A");
        let modern = n("B").with_new_format();
        assert_eq!(select_stream_format(&modern, true), StreamFormat::NewFormat);
        assert_eq!(select_stream_format(&modern, false), StreamFormat::Standard);
        assert_eq!(select_stream_format(&plain, true), StreamFormat::Standard);
        assert_eq!(select_stream_format(&plain, false), StreamFormat::Standard);
    }

    #[test]
    fn diff_reports_appeared_and_vanished_sources_by_name() {
        let d = diff_sources(&[n("A"), n("B")], &[n("B"), n("C")]);
        assert_eq!(d.discovered, [n("C")]);
        assert_eq!(d.lost, ["A"]);
        assert_eq!(diff_sources(&[n("A")], &[n("A")]), SourceDiff::default());
    }

    #[test]
    fn diff_ignores_metadata_changes_on_the_same_name() {
        let mut changed = n("A");
        changed.url = Some("10.0.0.5:5961".into());
        assert_eq!(diff_sources(&[n("A")], &[changed]), SourceDiff::default());
    }

    #[test]
    fn normalize_sorts_and_drops_duplicate_names() {
        let out = normalize(vec![n("b"), n("a"), n("b")]);
        assert_eq!(out, [n("a"), n("b")]);
    }

    #[test]
    fn description_quotes_awkward_names() {
        let d = ndi_source_description(r#"CAM "1" (Mevo\Core)"#);
        assert!(d.starts_with(r#"ndisrc ndi-name="CAM \"1\" (Mevo\\Core)" ! queue ! "#));
        assert!(d.contains("videoscale name=scale"));
        assert!(d.contains("width=320,height=180 ! appsink name=sink"));
    }

    #[test]
    fn simulated_discovery_scripts_the_network() {
        let sim = SimulatedDiscovery::default();
        sim.appear(n("A"));
        sim.appear(n("A").with_new_format());
        assert_eq!(sim.list().expect("list"), [n("A").with_new_format()]);
        sim.disappear("A");
        assert!(sim.list().expect("list").is_empty());
        sim.fail_with(Some("down"));
        assert!(matches!(sim.list(), Err(AppError::Internal(m)) if m == "down"));
    }

    #[test]
    fn unavailable_discovery_lists_nothing_and_says_why() {
        let u = UnavailableDiscovery("no sdk".into());
        assert!(u.list().expect("list").is_empty());
        assert_eq!(u.unavailable_reason().as_deref(), Some("no sdk"));
    }

    struct Rig {
        sim: SimulatedDiscovery,
        provider: NdiDiscoveryHandle,
        backend: SimulatedBackend,
        pipeline: PipelineHandle,
        bus: Arc<EventBus>,
        state: NdiStateHandle,
    }

    fn rig() -> Rig {
        let (bus, _rx) = EventBus::new();
        let bus = Arc::new(bus);
        let backend = SimulatedBackend::default();
        let sim = SimulatedDiscovery::default();
        Rig {
            provider: Arc::new(sim.clone()),
            sim,
            pipeline: PipelineService::spawn(Box::new(backend.clone()), Arc::clone(&bus)),
            backend,
            bus,
            state: new_ndi_handle(),
        }
    }

    impl Rig {
        async fn poll(&self) {
            reconcile_sources(&self.provider, &self.pipeline, &self.bus, &self.state).await;
        }
    }

    fn drain(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> Vec<AppEvent> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e);
        }
        out
    }

    #[tokio::test]
    async fn poll_publishes_discovery_then_nothing_when_unchanged() {
        let r = rig();
        let mut rx = r.bus.subscribe();
        r.sim.set(vec![n("Cam A")]);
        r.poll().await;
        let ev = drain(&mut rx);
        assert!(matches!(&ev[0], AppEvent::SourceDiscovered { source } if source.name == "Cam A"));
        assert!(matches!(&ev[1], AppEvent::SourceListChanged { sources } if sources.len() == 1));
        r.poll().await;
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn failed_scan_keeps_known_sources_and_publishes_nothing() {
        let r = rig();
        r.sim.set(vec![n("A")]);
        r.poll().await;
        let mut rx = r.bus.subscribe();
        r.sim.fail_with(Some("down"));
        r.poll().await;
        assert!(drain(&mut rx).is_empty());
        assert_eq!(r.state.read().await.discovered, [n("A")]);
    }

    #[tokio::test]
    async fn add_rejects_unknown_and_duplicate_sources() {
        let r = rig();
        let add = |name: &'static str| {
            let (p, pl, b, s) = (&r.provider, &r.pipeline, &r.bus, &r.state);
            async move { add_source(name, p, pl, b, s).await }
        };
        assert!(matches!(add("A").await, Err(AppError::NotFound(_))));
        r.sim.set(vec![n("A")]);
        r.poll().await;
        add("A").await.expect("add");
        assert!(matches!(add("A").await, Err(AppError::InvalidInput(_))));
        assert_eq!(r.backend.calls(), ["add_source:A:Standard"]);
    }

    #[tokio::test]
    async fn backend_add_failure_leaves_the_source_unadded() {
        let r = rig();
        r.sim.set(vec![n("A")]);
        r.poll().await;
        r.backend.fail_next_add_source("no ndisrc");
        let e = add_source("A", &r.provider, &r.pipeline, &r.bus, &r.state).await;
        assert!(matches!(e, Err(AppError::Internal(m)) if m == "no ndisrc"));
        assert!(r.state.read().await.added.is_empty());
    }

    #[tokio::test]
    async fn lost_added_source_is_torn_down_marked_lost_and_reattached_on_return() {
        let r = rig();
        r.sim.set(vec![n("A")]);
        r.poll().await;
        add_source("A", &r.provider, &r.pipeline, &r.bus, &r.state)
            .await
            .expect("add");
        let mut rx = r.bus.subscribe();

        r.sim.disappear("A");
        r.poll().await;
        let ev = drain(&mut rx);
        assert!(matches!(&ev[0], AppEvent::SourceLost { name } if name == "A"));
        assert!(r.state.read().await.added[0].lost);
        assert_eq!(
            r.backend.calls(),
            ["add_source:A:Standard", "remove_source:A"]
        );

        r.sim.appear(n("A"));
        r.poll().await;
        assert!(!r.state.read().await.added[0].lost);
        assert_eq!(
            r.backend.calls().last().map(String::as_str),
            Some("add_source:A:Standard")
        );
    }

    #[tokio::test]
    async fn failed_reattach_retries_on_the_next_poll() {
        let r = rig();
        r.sim.set(vec![n("A")]);
        r.poll().await;
        add_source("A", &r.provider, &r.pipeline, &r.bus, &r.state)
            .await
            .expect("add");
        r.sim.disappear("A");
        r.poll().await;
        r.sim.appear(n("A"));
        r.backend.fail_next_add_source("busy");
        let mut rx = r.bus.subscribe();
        r.poll().await;
        assert!(r.state.read().await.added[0].lost);
        assert!(drain(&mut rx)
            .iter()
            .any(|e| matches!(e, AppEvent::PipelineError { .. })));
        r.poll().await;
        assert!(!r.state.read().await.added[0].lost);
    }

    #[tokio::test]
    async fn remove_unknown_is_not_found_and_remove_publishes() {
        let r = rig();
        let e = remove_source("A", &r.pipeline, &r.bus, &r.state).await;
        assert!(matches!(e, Err(AppError::NotFound(_))));
        r.sim.set(vec![n("A")]);
        r.poll().await;
        add_source("A", &r.provider, &r.pipeline, &r.bus, &r.state)
            .await
            .expect("add");
        let mut rx = r.bus.subscribe();
        remove_source("A", &r.pipeline, &r.bus, &r.state)
            .await
            .expect("remove");
        assert!(matches!(&drain(&mut rx)[0], AppEvent::SourceRemoved { name } if name == "A"));
        assert!(r.state.read().await.added.is_empty());
    }

    #[tokio::test]
    async fn new_format_is_chosen_only_when_advertised_and_supported() {
        let r = rig();
        r.sim.set(vec![n("Modern").with_new_format(), n("Old")]);
        r.sim.set_new_format_runtime(true);
        r.poll().await;
        for name in ["Modern", "Old"] {
            add_source(name, &r.provider, &r.pipeline, &r.bus, &r.state)
                .await
                .expect("add");
        }
        assert_eq!(
            r.backend.calls(),
            ["add_source:Modern:NewFormat", "add_source:Old:Standard"]
        );
    }
}
