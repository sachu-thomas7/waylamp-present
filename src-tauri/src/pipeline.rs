use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use crate::bus::EventBus;
use crate::compositor::{self, CompositorInfo, CompositorStatus, SourceGeometry};
use crate::error::AppError;
use crate::events::AppEvent;
use crate::ndi::StreamFormat;
use crate::preview::{PreviewHub, SimulatedFeeds};

pub const TEST_PIPELINE: &str = "videotestsrc is-live=true ! autovideosink";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum PipelineState {
    Null,
    Paused,
    Playing,
}

/// The media engine behind the actor; runs only on the actor's blocking thread.
pub trait PipelineBackend: Send + 'static {
    fn start(&mut self, description: &str) -> Result<(), AppError>;
    fn pause(&mut self) -> Result<(), AppError>;
    fn resume(&mut self) -> Result<(), AppError>;
    /// Must reach NULL and be idempotent.
    fn stop(&mut self) -> Result<(), AppError>;
    /// Attaches one NDI source alongside the test pipeline; never disturbs what is running.
    fn add_ndi_source(&mut self, name: &str, format: StreamFormat) -> Result<(), AppError>;
    /// Idempotent: removing an unknown source is fine.
    fn remove_ndi_source(&mut self, name: &str) -> Result<(), AppError>;
    /// Starts the compositor pipeline with `layout`; independent of the test pipeline.
    fn compositor_start(&mut self, layout: &[SourceGeometry]) -> Result<CompositorInfo, AppError>;
    /// Adds, moves and removes sources of the running compositor without restarting it.
    fn compositor_set_layout(&mut self, layout: &[SourceGeometry]) -> Result<(), AppError>;
    /// Must reach NULL and be idempotent.
    fn compositor_stop(&mut self) -> Result<(), AppError>;
    /// Final teardown at app exit: everything reaches NULL.
    fn shutdown(&mut self) {
        let _ = self.compositor_stop();
        let _ = self.stop();
    }
}

type Reply = oneshot::Sender<Result<(), AppError>>;

enum Command {
    Start(String, Reply),
    Pause(Reply),
    Resume(Reply),
    Stop(Reply),
    AddSource(String, StreamFormat, Reply),
    RemoveSource(String, Reply),
    GetState(oneshot::Sender<PipelineState>),
    CompositorStart(Vec<SourceGeometry>, Reply),
    CompositorSetLayout(Vec<SourceGeometry>, Reply),
    CompositorStop(Reply),
    CompositorStatus(oneshot::Sender<CompositorStatus>),
    Shutdown(std::sync::mpsc::Sender<()>),
}

/// Handle to the pipeline actor; all state lives on the actor's thread.
pub struct PipelineService {
    tx: mpsc::UnboundedSender<Command>,
}

pub type PipelineHandle = Arc<PipelineService>;

fn gone() -> AppError {
    AppError::Internal("pipeline service is not running".into())
}

fn invalid(action: &str, state: PipelineState) -> AppError {
    AppError::InvalidInput(format!("cannot {action} while pipeline is {state:?}"))
}

impl PipelineService {
    /// Runs the actor on a blocking thread; safe to call from `.setup()` (no Tokio context needed).
    pub fn spawn(backend: Box<dyn PipelineBackend>, bus: Arc<EventBus>) -> PipelineHandle {
        let (tx, rx) = mpsc::unbounded_channel();
        tauri::async_runtime::spawn_blocking(move || run_actor(backend, bus, rx));
        Arc::new(Self { tx })
    }

    async fn request<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<T>) -> Command,
    ) -> Result<T, AppError> {
        let (reply, rx) = oneshot::channel();
        self.tx.send(make(reply)).map_err(|_| gone())?;
        rx.await.map_err(|_| gone())
    }

    pub async fn start(&self, description: &str) -> Result<(), AppError> {
        let d = description.to_string();
        self.request(|r| Command::Start(d, r)).await?
    }

    pub async fn pause(&self) -> Result<(), AppError> {
        self.request(Command::Pause).await?
    }

    pub async fn resume(&self) -> Result<(), AppError> {
        self.request(Command::Resume).await?
    }

    pub async fn stop(&self) -> Result<(), AppError> {
        self.request(Command::Stop).await?
    }

    pub async fn add_ndi_source(&self, name: &str, format: StreamFormat) -> Result<(), AppError> {
        let n = name.to_string();
        self.request(|r| Command::AddSource(n, format, r)).await?
    }

    pub async fn remove_ndi_source(&self, name: &str) -> Result<(), AppError> {
        let n = name.to_string();
        self.request(|r| Command::RemoveSource(n, r)).await?
    }

    pub async fn state(&self) -> Result<PipelineState, AppError> {
        self.request(Command::GetState).await
    }

    pub async fn compositor_start(&self, layout: Vec<SourceGeometry>) -> Result<(), AppError> {
        self.request(|r| Command::CompositorStart(layout, r))
            .await?
    }

    pub async fn compositor_set_layout(&self, layout: Vec<SourceGeometry>) -> Result<(), AppError> {
        self.request(|r| Command::CompositorSetLayout(layout, r))
            .await?
    }

    pub async fn compositor_stop(&self) -> Result<(), AppError> {
        self.request(Command::CompositorStop).await?
    }

    pub async fn compositor_status(&self) -> Result<CompositorStatus, AppError> {
        self.request(Command::CompositorStatus).await
    }

    /// Drives the pipeline to NULL and ends the actor; for app exit. Returns false on timeout.
    pub fn shutdown_blocking(&self, timeout: Duration) -> bool {
        let (ack, done) = std::sync::mpsc::channel();
        self.tx.send(Command::Shutdown(ack)).is_ok() && done.recv_timeout(timeout).is_ok()
    }
}

fn run_actor(
    mut backend: Box<dyn PipelineBackend>,
    bus: Arc<EventBus>,
    mut rx: mpsc::UnboundedReceiver<Command>,
) {
    let mut state = PipelineState::Null;
    let set = |state: &mut PipelineState, next: PipelineState| {
        if *state != next {
            *state = next;
            bus.publish(AppEvent::PipelineStateChanged { state: next });
        }
    };
    let mut comp = CompositorStatus::default();
    let comp_stopped = |comp: &mut CompositorStatus| {
        if comp.running {
            *comp = CompositorStatus::default();
            bus.publish(AppEvent::CompositorStateChanged {
                running: false,
                element: String::new(),
                hardware: false,
            });
        }
    };
    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            Command::CompositorStart(layout, reply) => {
                let r = if comp.running {
                    Err(AppError::InvalidInput("compositor already running".into()))
                } else if layout.is_empty() {
                    Err(AppError::InvalidInput(
                        "compositor needs at least one source".into(),
                    ))
                } else {
                    compositor::validate_layout(&layout)
                        .and_then(|_| backend.compositor_start(&layout))
                };
                let r = r.map(|info| {
                    comp = CompositorStatus {
                        running: true,
                        element: Some(info.element.clone()),
                        hardware: info.hardware,
                        sources: layout.clone(),
                        ..CompositorStatus::default()
                    };
                    bus.publish(AppEvent::CompositorStateChanged {
                        running: true,
                        element: info.element,
                        hardware: info.hardware,
                    });
                    bus.publish(AppEvent::CompositorLayoutChanged { sources: layout });
                });
                let _ = reply.send(r);
            }
            Command::CompositorSetLayout(layout, reply) => {
                let r = if !comp.running {
                    Err(AppError::InvalidInput("compositor is not running".into()))
                } else {
                    compositor::validate_layout(&layout)
                        .and_then(|_| backend.compositor_set_layout(&layout))
                };
                let r = r.map(|_| {
                    comp.sources = layout.clone();
                    bus.publish(AppEvent::CompositorLayoutChanged { sources: layout });
                });
                let _ = reply.send(r);
            }
            Command::CompositorStop(reply) => {
                let r = backend.compositor_stop();
                if r.is_ok() {
                    comp_stopped(&mut comp);
                }
                let _ = reply.send(r);
            }
            Command::CompositorStatus(reply) => {
                let _ = reply.send(comp.clone());
            }
            Command::Start(desc, reply) => {
                let r = if state != PipelineState::Null {
                    Err(invalid("start", state))
                } else {
                    backend.start(&desc)
                };
                if r.is_ok() {
                    set(&mut state, PipelineState::Playing);
                }
                let _ = reply.send(r);
            }
            Command::Pause(reply) => {
                let r = if state != PipelineState::Playing {
                    Err(invalid("pause", state))
                } else {
                    backend.pause()
                };
                if r.is_ok() {
                    set(&mut state, PipelineState::Paused);
                }
                let _ = reply.send(r);
            }
            Command::Resume(reply) => {
                let r = if state != PipelineState::Paused {
                    Err(invalid("resume", state))
                } else {
                    backend.resume()
                };
                if r.is_ok() {
                    set(&mut state, PipelineState::Playing);
                }
                let _ = reply.send(r);
            }
            Command::Stop(reply) => {
                let r = backend.stop();
                if r.is_ok() {
                    set(&mut state, PipelineState::Null);
                }
                let _ = reply.send(r);
            }
            Command::AddSource(name, format, reply) => {
                let _ = reply.send(backend.add_ndi_source(&name, format));
            }
            Command::RemoveSource(name, reply) => {
                let _ = reply.send(backend.remove_ndi_source(&name));
            }
            Command::GetState(reply) => {
                let _ = reply.send(state);
            }
            Command::Shutdown(ack) => {
                backend.shutdown();
                comp_stopped(&mut comp);
                set(&mut state, PipelineState::Null);
                let _ = ack.send(());
                return;
            }
        }
    }
    backend.shutdown();
}

/// Used when GStreamer is not compiled in or failed to initialise.
pub struct UnavailableBackend(pub String);

impl PipelineBackend for UnavailableBackend {
    fn start(&mut self, _: &str) -> Result<(), AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn pause(&mut self) -> Result<(), AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn resume(&mut self) -> Result<(), AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn stop(&mut self) -> Result<(), AppError> {
        Ok(())
    }
    fn add_ndi_source(&mut self, _: &str, _: StreamFormat) -> Result<(), AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn remove_ndi_source(&mut self, _: &str) -> Result<(), AppError> {
        Ok(())
    }
    fn compositor_start(&mut self, _: &[SourceGeometry]) -> Result<CompositorInfo, AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn compositor_set_layout(&mut self, _: &[SourceGeometry]) -> Result<(), AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn compositor_stop(&mut self) -> Result<(), AppError> {
        Ok(())
    }
}

/// In-memory backend for tests and the mock harness: records calls, can fail the next start.
#[derive(Clone, Default)]
pub struct SimulatedBackend {
    pub log: Arc<Mutex<Vec<String>>>,
    pub fail_start: Arc<Mutex<Option<String>>>,
    pub fail_add_source: Arc<Mutex<Option<String>>>,
    pub sources: Arc<Mutex<Vec<String>>>,
    pub compositor_sources: Arc<Mutex<Vec<String>>>,
    pub fail_compositor_start: Arc<Mutex<Option<String>>>,
    feeds: Option<Arc<Mutex<SimulatedFeeds>>>,
}

impl SimulatedBackend {
    /// Attached sources push synthetic 320x180 test-pattern frames into `hub` at `fps`.
    pub fn with_preview(hub: PreviewHub, fps: u32) -> Self {
        Self {
            feeds: Some(Arc::new(Mutex::new(SimulatedFeeds::new(hub, fps)))),
            ..Self::default()
        }
    }

    fn with_feeds(&self, f: impl FnOnce(&mut SimulatedFeeds)) {
        if let Some(Ok(mut feeds)) = self.feeds.as_ref().map(|m| m.lock()) {
            f(&mut feeds);
        }
    }

    fn set_compositor_sources(&self, layout: &[SourceGeometry]) {
        if let Ok(mut s) = self.compositor_sources.lock() {
            *s = layout.iter().map(|g| g.name.clone()).collect();
        }
    }

    fn record(&self, entry: &str) {
        if let Ok(mut log) = self.log.lock() {
            log.push(entry.to_string());
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }

    pub fn fail_next_add_source(&self, reason: &str) {
        if let Ok(mut f) = self.fail_add_source.lock() {
            *f = Some(reason.to_string());
        }
    }

    /// Names of the NDI sources currently attached.
    pub fn attached_sources(&self) -> Vec<String> {
        self.sources.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn fail_next_compositor_start(&self, reason: &str) {
        if let Ok(mut f) = self.fail_compositor_start.lock() {
            *f = Some(reason.to_string());
        }
    }

    /// Names of the sources currently on the simulated compositor.
    pub fn compositor_attached(&self) -> Vec<String> {
        self.compositor_sources
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    pub fn fail_next_start(&self, reason: &str) {
        if let Ok(mut f) = self.fail_start.lock() {
            *f = Some(reason.to_string());
        }
    }
}

impl PipelineBackend for SimulatedBackend {
    fn start(&mut self, description: &str) -> Result<(), AppError> {
        self.record(&format!("start:{description}"));
        match self.fail_start.lock().ok().and_then(|mut f| f.take()) {
            Some(reason) => Err(AppError::Internal(reason)),
            None => Ok(()),
        }
    }
    fn pause(&mut self) -> Result<(), AppError> {
        self.record("pause");
        Ok(())
    }
    fn resume(&mut self) -> Result<(), AppError> {
        self.record("resume");
        Ok(())
    }
    fn stop(&mut self) -> Result<(), AppError> {
        self.record("stop");
        Ok(())
    }
    fn add_ndi_source(&mut self, name: &str, format: StreamFormat) -> Result<(), AppError> {
        self.record(&format!("add_source:{name}:{format:?}"));
        if let Some(reason) = self.fail_add_source.lock().ok().and_then(|mut f| f.take()) {
            return Err(AppError::Internal(reason));
        }
        let mut sources = self
            .sources
            .lock()
            .map_err(|_| AppError::Internal("sources lock poisoned".into()))?;
        if sources.iter().any(|s| s == name) {
            return Err(AppError::InvalidInput(format!(
                "source already attached: {name}"
            )));
        }
        sources.push(name.to_string());
        drop(sources);
        self.with_feeds(|f| f.start(name));
        Ok(())
    }
    fn compositor_start(&mut self, layout: &[SourceGeometry]) -> Result<CompositorInfo, AppError> {
        self.record(&format!("compositor_start:{}", layout.len()));
        if let Some(reason) = self
            .fail_compositor_start
            .lock()
            .ok()
            .and_then(|mut f| f.take())
        {
            return Err(AppError::Internal(reason));
        }
        self.set_compositor_sources(layout);
        Ok(CompositorInfo {
            element: "compositor".into(),
            hardware: false,
        })
    }
    fn compositor_set_layout(&mut self, layout: &[SourceGeometry]) -> Result<(), AppError> {
        self.record(&format!("compositor_layout:{}", layout.len()));
        self.set_compositor_sources(layout);
        Ok(())
    }
    fn compositor_stop(&mut self) -> Result<(), AppError> {
        self.record("compositor_stop");
        self.set_compositor_sources(&[]);
        Ok(())
    }
    fn shutdown(&mut self) {
        self.with_feeds(|f| f.stop_all());
        let _ = self.compositor_stop();
        let _ = self.stop();
    }
    fn remove_ndi_source(&mut self, name: &str) -> Result<(), AppError> {
        self.record(&format!("remove_source:{name}"));
        if let Ok(mut sources) = self.sources.lock() {
            sources.retain(|s| s != name);
        }
        self.with_feeds(|f| f.stop(name));
        Ok(())
    }
}

#[cfg(feature = "gstreamer")]
pub mod gst_backend {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::JoinHandle;

    use gstreamer as gst;
    use gstreamer::prelude::*;

    use super::*;
    use crate::compositor::gst_compositor::GstCompositor;
    use crate::compositor::OutputSink;
    use crate::preview::RawFrame;

    const WATCHED: &[gst::MessageType] = &[
        gst::MessageType::Error,
        gst::MessageType::Warning,
        gst::MessageType::Latency,
        gst::MessageType::Qos,
    ];

    pub(crate) struct Running {
        pub(crate) pipeline: gst::Pipeline,
        stop_flag: Arc<AtomicBool>,
        watcher: Option<JoinHandle<()>>,
    }

    /// Parses `description` and starts its bus watcher; the pipeline is still NULL.
    pub(crate) fn build_running(
        description: &str,
        events: Arc<EventBus>,
    ) -> Result<Running, AppError> {
        let pipeline = gst::parse::launch(description)
            .map_err(|e| AppError::InvalidInput(format!("invalid pipeline: {e}")))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| AppError::InvalidInput("pipeline needs 2+ linked elements".into()))?;
        let bus = pipeline
            .bus()
            .ok_or_else(|| AppError::Internal("pipeline has no bus".into()))?;
        let stop_flag = Arc::new(AtomicBool::new(false));
        let watcher = {
            let (p, flag) = (pipeline.clone(), stop_flag.clone());
            std::thread::Builder::new()
                .name("gst-bus-watcher".into())
                .spawn(move || watch(p, bus, events, flag))
                .map_err(|e| AppError::Internal(format!("watcher thread: {e}")))?
        };
        Ok(Running {
            pipeline,
            stop_flag,
            watcher: Some(watcher),
        })
    }

    type Availability = Box<dyn Fn(&str) -> bool + Send>;

    pub struct GstBackend {
        bus: Arc<EventBus>,
        preview: PreviewHub,
        running: Option<Running>,
        sources: std::collections::HashMap<String, Running>,
        compositor: Option<GstCompositor>,
        compositor_sink: OutputSink,
        available: Availability,
    }

    impl GstBackend {
        pub fn new(bus: Arc<EventBus>, preview: PreviewHub) -> Result<Self, AppError> {
            gst::init().map_err(|e| AppError::Internal(format!("GStreamer init failed: {e}")))?;
            Ok(Self {
                bus,
                preview,
                running: None,
                sources: Default::default(),
                compositor: None,
                compositor_sink: OutputSink::Display,
                available: Box::new(crate::compositor::gst_compositor::factory_available),
            })
        }

        pub fn with_compositor_sink(mut self, sink: OutputSink) -> Self {
            self.compositor_sink = sink;
            self
        }

        /// Overrides which element factories count as installed; for testing the CPU fallback.
        pub fn with_element_filter(
            mut self,
            available: impl Fn(&str) -> bool + Send + 'static,
        ) -> Self {
            self.available = Box::new(available);
            self
        }

        pub fn compositor(&self) -> Option<&GstCompositor> {
            self.compositor.as_ref()
        }

        fn build(&self, description: &str) -> Result<Running, AppError> {
            build_running(description, self.bus.clone())
        }

        /// Runs `description` as source `name`, feeding its appsink into the preview hub.
        pub(crate) fn attach_source(
            &mut self,
            name: &str,
            description: &str,
        ) -> Result<(), AppError> {
            if self.sources.contains_key(name) {
                return Err(AppError::InvalidInput(format!(
                    "source already attached: {name}"
                )));
            }
            let r = self.build(description)?;
            attach_preview(&r.pipeline, name, self.preview.clone());
            if let Err(e) = r.pipeline.set_state(gst::State::Playing) {
                let _ = teardown(r);
                return Err(AppError::Internal(format!("source failed to start: {e}")));
            }
            self.sources.insert(name.to_string(), r);
            Ok(())
        }

        fn set_state(&self, state: gst::State) -> Result<(), AppError> {
            let r = self
                .running
                .as_ref()
                .ok_or_else(|| AppError::InvalidInput("no pipeline".into()))?;
            r.pipeline
                .set_state(state)
                .map(|_| ())
                .map_err(|e| AppError::Internal(format!("state change to {state:?} failed: {e}")))
        }
    }

    fn forward(pipeline: &gst::Pipeline, events: &EventBus, msg: &gst::Message) {
        use gst::MessageView;
        let element = msg.src().map(|s| s.name().to_string()).unwrap_or_default();
        let detail = |err: &gst::glib::Error, debug: Option<gst::glib::GString>| match debug {
            Some(d) => format!("{err} ({d})"),
            None => err.to_string(),
        };
        let event = match msg.view() {
            MessageView::Error(e) => AppEvent::PipelineError {
                element,
                message: detail(&e.error(), e.debug()),
            },
            MessageView::Warning(w) => AppEvent::PipelineWarning {
                element,
                message: detail(&w.error(), w.debug()),
            },
            MessageView::Latency(_) => {
                let _ = pipeline.recalculate_latency();
                AppEvent::PipelineLatency { element }
            }
            MessageView::Qos(q) => {
                let (processed, dropped) = q.stats();
                AppEvent::PipelineQos {
                    element,
                    processed: processed.value().max(0) as u64,
                    dropped: dropped.value().max(0) as u64,
                }
            }
            _ => return,
        };
        events.publish(event);
    }

    fn watch(pipeline: gst::Pipeline, bus: gst::Bus, events: Arc<EventBus>, stop: Arc<AtomicBool>) {
        while !stop.load(Ordering::Acquire) {
            if let Some(msg) = bus.timed_pop_filtered(gst::ClockTime::from_mseconds(20), WATCHED) {
                forward(&pipeline, &events, &msg);
            }
        }
        while let Some(msg) = bus.pop_filtered(WATCHED) {
            forward(&pipeline, &events, &msg);
        }
    }

    impl PipelineBackend for GstBackend {
        fn start(&mut self, description: &str) -> Result<(), AppError> {
            if self.running.is_some() {
                return Err(AppError::InvalidInput("pipeline already running".into()));
            }
            let r = self.build(description)?;
            let started = r.pipeline.set_state(gst::State::Playing);
            self.running = Some(r);
            if let Err(e) = started {
                let _ = self.stop();
                return Err(AppError::Internal(format!("pipeline failed to start: {e}")));
            }
            Ok(())
        }

        fn pause(&mut self) -> Result<(), AppError> {
            self.set_state(gst::State::Paused)
        }

        fn resume(&mut self) -> Result<(), AppError> {
            self.set_state(gst::State::Playing)
        }

        fn stop(&mut self) -> Result<(), AppError> {
            match self.running.take() {
                Some(r) => teardown(r),
                None => Ok(()),
            }
        }

        fn add_ndi_source(&mut self, name: &str, format: StreamFormat) -> Result<(), AppError> {
            // `format` does not change the element yet; New Format needs real-SDK validation.
            let _ = format;
            self.attach_source(name, &crate::ndi::ndi_source_description(name))
        }

        fn remove_ndi_source(&mut self, name: &str) -> Result<(), AppError> {
            let result = match self.sources.remove(name) {
                Some(r) => teardown(r),
                None => Ok(()),
            };
            self.preview.forget(name);
            result
        }

        fn compositor_start(
            &mut self,
            layout: &[SourceGeometry],
        ) -> Result<CompositorInfo, AppError> {
            if self.compositor.is_some() {
                return Err(AppError::InvalidInput("compositor already running".into()));
            }
            let c = GstCompositor::start(
                self.bus.clone(),
                layout,
                self.compositor_sink,
                self.available.as_ref(),
            )?;
            let info = c.info.clone();
            self.compositor = Some(c);
            Ok(info)
        }

        fn compositor_set_layout(&mut self, layout: &[SourceGeometry]) -> Result<(), AppError> {
            self.compositor
                .as_mut()
                .ok_or_else(|| AppError::InvalidInput("compositor is not running".into()))?
                .set_layout(layout)
        }

        fn compositor_stop(&mut self) -> Result<(), AppError> {
            match self.compositor.take() {
                Some(c) => c.stop(),
                None => Ok(()),
            }
        }

        fn shutdown(&mut self) {
            for (_, r) in self.sources.drain() {
                let _ = teardown(r);
            }
            let _ = self.compositor_stop();
            let _ = self.stop();
        }
    }

    /// Best-effort and untested without a real NDI source: tracks the source resolution and feeds
    /// 320x180 RGBA samples to the hub; latency is the buffer PTS vs the pipeline running time.
    pub(crate) fn attach_preview(pipeline: &gst::Pipeline, name: &str, hub: PreviewHub) {
        let native = Arc::new(Mutex::new((0u32, 0u32)));
        if let Some(pad) = pipeline.by_name("scale").and_then(|e| e.static_pad("sink")) {
            let native = native.clone();
            pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
                if let Some(gst::PadProbeData::Event(ev)) = &info.data {
                    if let gst::EventView::Caps(c) = ev.view() {
                        let size = c.caps().structure(0).and_then(|s| {
                            Some((s.get::<i32>("width").ok()?, s.get::<i32>("height").ok()?))
                        });
                        if let (Some((w, h)), Ok(mut n)) = (size, native.lock()) {
                            *n = (w.max(0) as u32, h.max(0) as u32);
                        }
                    }
                }
                gst::PadProbeReturn::Ok
            });
        }
        let Some(sink) = pipeline
            .by_name("sink")
            .and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok())
        else {
            return;
        };
        let name = name.to_string();
        sink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |s| {
                    let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let delay_ms = match (buffer.pts(), s.current_running_time()) {
                        (Some(pts), Some(now)) => now.saturating_sub(pts).mseconds().min(5_000),
                        _ => 0,
                    };
                    let (w, h) = native.lock().map(|n| *n).unwrap_or((0, 0));
                    hub.push(RawFrame {
                        source: name.clone(),
                        native_width: w,
                        native_height: h,
                        rgba: map.as_slice().to_vec(),
                        source_delay_ms: delay_ms as u32,
                    });
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    pub(crate) fn teardown(mut r: Running) -> Result<(), AppError> {
        // Drain the watcher before NULL: leaving PLAYING flushes the bus and would drop errors.
        r.stop_flag.store(true, Ordering::Release);
        if let Some(h) = r.watcher.take() {
            let _ = h.join();
        }
        r.pipeline
            .set_state(gst::State::Null)
            .map(|_| ())
            .map_err(|e| AppError::Internal(format!("failed to reach NULL: {e}")))
    }

    impl Drop for GstBackend {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    fn service() -> (PipelineHandle, SimulatedBackend, Arc<EventBus>) {
        let (bus, _rx) = EventBus::new();
        let bus = Arc::new(bus);
        let backend = SimulatedBackend::default();
        let svc = PipelineService::spawn(Box::new(backend.clone()), Arc::clone(&bus));
        (svc, backend, bus)
    }

    fn states(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> Vec<PipelineState> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            if let AppEvent::PipelineStateChanged { state } = e {
                out.push(state);
            }
        }
        out
    }

    #[tokio::test]
    async fn lifecycle_publishes_each_state_change() {
        let (svc, _b, bus) = service();
        let mut rx = bus.subscribe();
        svc.start("a ! b").await.expect("start");
        svc.pause().await.expect("pause");
        svc.resume().await.expect("resume");
        svc.stop().await.expect("stop");
        assert_eq!(svc.state().await.expect("state"), PipelineState::Null);
        assert_eq!(
            states(&mut rx),
            [
                PipelineState::Playing,
                PipelineState::Paused,
                PipelineState::Playing,
                PipelineState::Null
            ]
        );
    }

    #[tokio::test]
    async fn invalid_transitions_are_rejected_without_touching_the_backend() {
        let (svc, backend, _bus) = service();
        assert!(matches!(svc.pause().await, Err(AppError::InvalidInput(_))));
        assert!(matches!(svc.resume().await, Err(AppError::InvalidInput(_))));
        svc.start("a ! b").await.expect("start");
        assert!(matches!(
            svc.start("a ! b").await,
            Err(AppError::InvalidInput(_))
        ));
        assert!(matches!(svc.resume().await, Err(AppError::InvalidInput(_))));
        assert_eq!(backend.calls(), ["start:a ! b"]);
    }

    #[tokio::test]
    async fn stop_when_idle_is_a_quiet_no_op() {
        let (svc, _b, bus) = service();
        let mut rx = bus.subscribe();
        svc.stop().await.expect("stop");
        assert!(states(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn failed_start_stays_null_and_can_retry() {
        let (svc, backend, _bus) = service();
        backend.fail_next_start("boom");
        assert!(matches!(svc.start("a ! b").await, Err(AppError::Internal(m)) if m == "boom"));
        assert_eq!(svc.state().await.expect("state"), PipelineState::Null);
        svc.start("a ! b").await.expect("retry");
        assert_eq!(svc.state().await.expect("state"), PipelineState::Playing);
    }

    #[tokio::test]
    async fn start_pause_resume_stop_loop() {
        let (svc, backend, _bus) = service();
        for _ in 0..100 {
            svc.start("a ! b").await.expect("start");
            svc.pause().await.expect("pause");
            svc.resume().await.expect("resume");
            svc.stop().await.expect("stop");
        }
        assert_eq!(backend.calls().len(), 400);
    }

    #[tokio::test]
    async fn shutdown_stops_the_backend_publishes_null_and_ends_the_actor() {
        let (svc, backend, bus) = service();
        svc.start("a ! b").await.expect("start");
        let mut rx = bus.subscribe();
        let s = Arc::clone(&svc);
        let ok = tokio::task::spawn_blocking(move || s.shutdown_blocking(Duration::from_secs(2)))
            .await
            .expect("join");
        assert!(ok);
        assert_eq!(backend.calls().last().map(String::as_str), Some("stop"));
        assert_eq!(states(&mut rx), [PipelineState::Null]);
        let after = timeout(Duration::from_secs(1), svc.start("a ! b")).await;
        assert!(matches!(after, Ok(Err(AppError::Internal(_)))));
    }

    #[tokio::test]
    async fn dropping_the_handle_stops_a_running_backend() {
        let (svc, backend, _bus) = service();
        svc.start("a ! b").await.expect("start");
        drop(svc);
        for _ in 0..100 {
            if backend.calls().last().map(String::as_str) == Some("stop") {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("backend was never stopped: {:?}", backend.calls());
    }

    #[tokio::test]
    async fn unavailable_backend_reports_why() {
        let (bus, _rx) = EventBus::new();
        let svc =
            PipelineService::spawn(Box::new(UnavailableBackend("no gst".into())), Arc::new(bus));
        assert!(matches!(svc.start("a ! b").await, Err(AppError::Internal(m)) if m == "no gst"));
        svc.stop().await.expect("stop is always fine");
    }

    fn layout(names: &[&str]) -> Vec<SourceGeometry> {
        names
            .iter()
            .map(|n| SourceGeometry {
                name: (*n).into(),
                w: 640,
                h: 360,
                ..SourceGeometry::default()
            })
            .collect()
    }

    fn compositor_events(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> Vec<AppEvent> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            if matches!(
                e,
                AppEvent::CompositorStateChanged { .. } | AppEvent::CompositorLayoutChanged { .. }
            ) {
                out.push(e);
            }
        }
        out
    }

    #[tokio::test]
    async fn compositor_lifecycle_publishes_state_and_layout_and_tracks_status() {
        let (svc, backend, bus) = service();
        let mut rx = bus.subscribe();
        svc.compositor_start(layout(&["a", "b"]))
            .await
            .expect("start");
        svc.compositor_set_layout(layout(&["b", "c", "d"]))
            .await
            .expect("layout");
        let status = svc.compositor_status().await.expect("status");
        assert!(status.running);
        assert_eq!(status.element.as_deref(), Some("compositor"));
        assert_eq!(status.sources, layout(&["b", "c", "d"]));
        assert_eq!(backend.compositor_attached(), ["b", "c", "d"]);
        svc.compositor_stop().await.expect("stop");
        assert_eq!(
            svc.compositor_status().await.expect("status"),
            CompositorStatus::default()
        );
        let seen = compositor_events(&mut rx);
        assert!(matches!(
            &seen[..],
            [
                AppEvent::CompositorStateChanged { running: true, element, hardware: false },
                AppEvent::CompositorLayoutChanged { sources: first },
                AppEvent::CompositorLayoutChanged { sources: second },
                AppEvent::CompositorStateChanged { running: false, .. },
            ] if element == "compositor" && first.len() == 2 && second.len() == 3
        ));
    }

    #[tokio::test]
    async fn compositor_rejects_bad_input_without_touching_the_backend() {
        let (svc, backend, _bus) = service();
        let invalid =
            |r: Result<(), AppError>| assert!(matches!(r, Err(AppError::InvalidInput(_))));
        invalid(svc.compositor_set_layout(layout(&["a"])).await);
        invalid(svc.compositor_start(vec![]).await);
        invalid(svc.compositor_start(layout(&["a", "a"])).await);
        assert!(backend.calls().is_empty());
        svc.compositor_start(layout(&["a"])).await.expect("start");
        invalid(svc.compositor_start(layout(&["a"])).await);
        invalid(svc.compositor_set_layout(layout(&["x", "x"])).await);
        assert_eq!(backend.calls(), ["compositor_start:1"]);
        assert_eq!(backend.compositor_attached(), ["a"]);
    }

    #[tokio::test]
    async fn failed_compositor_start_stays_stopped_and_can_retry() {
        let (svc, backend, bus) = service();
        let mut rx = bus.subscribe();
        backend.fail_next_compositor_start("no gpu");
        assert!(matches!(
            svc.compositor_start(layout(&["a"])).await,
            Err(AppError::Internal(m)) if m == "no gpu"
        ));
        assert!(!svc.compositor_status().await.expect("status").running);
        assert!(compositor_events(&mut rx).is_empty());
        svc.compositor_start(layout(&["a"])).await.expect("retry");
    }

    #[tokio::test]
    async fn compositor_is_independent_of_the_test_pipeline_and_stops_on_shutdown() {
        let (svc, backend, bus) = service();
        svc.start("a ! b").await.expect("test pipeline");
        svc.compositor_start(layout(&["a"])).await.expect("start");
        svc.stop().await.expect("stop test pipeline");
        assert!(svc.compositor_status().await.expect("status").running);
        let mut rx = bus.subscribe();
        let s = Arc::clone(&svc);
        assert!(
            tokio::task::spawn_blocking(move || s.shutdown_blocking(Duration::from_secs(2)))
                .await
                .expect("join")
        );
        assert!(backend.compositor_attached().is_empty());
        assert!(matches!(
            &compositor_events(&mut rx)[..],
            [AppEvent::CompositorStateChanged { running: false, .. }]
        ));
    }

    #[tokio::test]
    async fn unavailable_backend_refuses_the_compositor_with_its_reason() {
        let (bus, _rx) = EventBus::new();
        let svc =
            PipelineService::spawn(Box::new(UnavailableBackend("no gst".into())), Arc::new(bus));
        assert!(matches!(
            svc.compositor_start(layout(&["a"])).await,
            Err(AppError::Internal(m)) if m == "no gst"
        ));
        svc.compositor_stop().await.expect("stop is always fine");
        assert!(!svc.compositor_status().await.expect("status").running);
    }

    #[test]
    fn compositor_events_serialise_with_type_and_payload() {
        let v = serde_json::to_value(AppEvent::CompositorStateChanged {
            running: true,
            element: "d3d12compositor".into(),
            hardware: true,
        })
        .expect("ser");
        assert_eq!(v["type"], "CompositorStateChanged");
        assert_eq!(v["payload"]["element"], "d3d12compositor");
        let v = serde_json::to_value(AppEvent::CompositorLayoutChanged {
            sources: layout(&["a"]),
        })
        .expect("ser");
        assert_eq!(v["payload"]["sources"][0]["kind"], "test");
    }

    #[test]
    fn pipeline_state_serialises_as_a_plain_string() {
        let v = serde_json::to_value(AppEvent::PipelineStateChanged {
            state: PipelineState::Paused,
        })
        .expect("ser");
        assert_eq!(v["type"], "PipelineStateChanged");
        assert_eq!(v["payload"]["state"], "Paused");
    }
}

#[cfg(all(test, feature = "gstreamer"))]
mod gst_tests {
    use super::gst_backend::GstBackend;
    use super::*;
    use std::time::Instant;

    const FAKE: &str = "videotestsrc is-live=true ! fakesink sync=true";

    fn service() -> (PipelineHandle, Arc<EventBus>) {
        let (bus, _rx) = EventBus::new();
        let bus = Arc::new(bus);
        let backend =
            GstBackend::new(Arc::clone(&bus), PreviewHub::new()).expect("gstreamer initialises");
        (
            PipelineService::spawn(Box::new(backend), Arc::clone(&bus)),
            bus,
        )
    }

    #[tokio::test]
    async fn real_pipeline_start_pause_resume_stop_loop() {
        let n: usize = std::env::var("STREAMDECK_PIPELINE_LOOPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let (svc, _bus) = service();
        for _ in 0..n {
            svc.start(FAKE).await.expect("start");
            svc.pause().await.expect("pause");
            svc.resume().await.expect("resume");
            svc.stop().await.expect("stop");
            assert_eq!(svc.state().await.expect("state"), PipelineState::Null);
        }
    }

    #[tokio::test]
    async fn runtime_error_surfaces_as_pipeline_error_quickly() {
        let (svc, bus) = service();
        let mut rx = bus.subscribe();
        let t = Instant::now();
        let r = svc
            .start("filesrc location=/definitely/not/here.bin ! fakesink")
            .await;
        let mut seen = None;
        while seen.is_none() {
            match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                Ok(Ok(AppEvent::PipelineError { element, message })) => {
                    seen = Some((element, message))
                }
                Ok(Ok(_)) => {}
                _ => break,
            }
        }
        let (element, message) = seen.expect("PipelineError was published");
        assert!(element.starts_with("filesrc"), "element: {element}");
        assert!(!message.is_empty());
        assert!(
            t.elapsed() < Duration::from_millis(500),
            "{:?}",
            t.elapsed()
        );
        let _ = r;
        assert_eq!(svc.state().await.expect("state"), PipelineState::Null);
    }

    #[tokio::test]
    async fn invalid_description_is_invalid_input() {
        let (svc, _bus) = service();
        let r = svc.start("videotestsrc ! no_such_element_xyz").await;
        assert!(matches!(r, Err(AppError::InvalidInput(_))), "{r:?}");
        assert_eq!(svc.state().await.expect("state"), PipelineState::Null);
    }

    // Same appsink segment as an NDI source, with videotestsrc standing in for ndisrc.
    #[tokio::test]
    async fn real_appsink_delivers_320x180_frames_throttled_with_native_size() {
        let (bus, _rx) = EventBus::new();
        let hub = crate::preview::PreviewHub::new();
        let mut backend =
            GstBackend::new(Arc::new(bus), hub.clone()).expect("gstreamer initialises");
        let mut rx = hub.subscribe();
        backend
            .attach_source(
                "fake",
                "videotestsrc is-live=true ! video/x-raw,width=640,height=360,framerate=30/1 ! videoconvert ! videoscale name=scale add-borders=true ! video/x-raw,format=RGBA,width=320,height=180 ! appsink name=sink max-buffers=2 drop=true sync=false",
            )
            .expect("attach");
        let mut frames = Vec::new();
        let _ = tokio::time::timeout(Duration::from_millis(2200), async {
            while let Ok(f) = rx.recv().await {
                frames.push(f);
            }
        })
        .await;
        let bus2 = EventBus::new().0;
        hub.flush_stats(&bus2, Instant::now());
        let snap = hub.snapshot();
        backend.shutdown();
        assert!((8..=19).contains(&frames.len()), "frames: {}", frames.len());
        assert!(frames.iter().all(|f| (f.width, f.height) == (320, 180)));
        assert_eq!(snap.stats[0].name, "fake");
        assert_eq!((snap.stats[0].width, snap.stats[0].height), (640, 360));
    }

    #[tokio::test]
    async fn shutdown_reaches_null_and_a_new_service_can_start_afterwards() {
        let (svc, _bus) = service();
        svc.start(FAKE).await.expect("start");
        let s = Arc::clone(&svc);
        assert!(
            tokio::task::spawn_blocking(move || s.shutdown_blocking(Duration::from_secs(3)))
                .await
                .expect("join")
        );
        let (svc2, _bus2) = service();
        svc2.start(FAKE).await.expect("start again");
        svc2.stop().await.expect("stop");
    }
}
