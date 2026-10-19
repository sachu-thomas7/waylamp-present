use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;

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
}

type Reply = oneshot::Sender<Result<(), AppError>>;

enum Command {
    Start(String, Reply),
    Pause(Reply),
    Resume(Reply),
    Stop(Reply),
    GetState(oneshot::Sender<PipelineState>),
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

    pub async fn state(&self) -> Result<PipelineState, AppError> {
        self.request(Command::GetState).await
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
    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
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
            Command::GetState(reply) => {
                let _ = reply.send(state);
            }
            Command::Shutdown(ack) => {
                let _ = backend.stop();
                set(&mut state, PipelineState::Null);
                let _ = ack.send(());
                return;
            }
        }
    }
    let _ = backend.stop();
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
}

/// In-memory backend for tests and the mock harness: records calls, can fail the next start.
#[derive(Clone, Default)]
pub struct SimulatedBackend {
    pub log: Arc<Mutex<Vec<String>>>,
    pub fail_start: Arc<Mutex<Option<String>>>,
}

impl SimulatedBackend {
    fn record(&self, entry: &str) {
        if let Ok(mut log) = self.log.lock() {
            log.push(entry.to_string());
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
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
}

#[cfg(feature = "gstreamer")]
pub mod gst_backend {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::JoinHandle;

    use gstreamer as gst;
    use gstreamer::prelude::*;

    use super::*;

    const WATCHED: &[gst::MessageType] = &[
        gst::MessageType::Error,
        gst::MessageType::Warning,
        gst::MessageType::Latency,
        gst::MessageType::Qos,
    ];

    struct Running {
        pipeline: gst::Pipeline,
        stop_flag: Arc<AtomicBool>,
        watcher: Option<JoinHandle<()>>,
    }

    pub struct GstBackend {
        bus: Arc<EventBus>,
        running: Option<Running>,
    }

    impl GstBackend {
        pub fn new(bus: Arc<EventBus>) -> Result<Self, AppError> {
            gst::init().map_err(|e| AppError::Internal(format!("GStreamer init failed: {e}")))?;
            Ok(Self { bus, running: None })
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
            let pipeline = gst::parse::launch(description)
                .map_err(|e| AppError::InvalidInput(format!("invalid pipeline: {e}")))?
                .downcast::<gst::Pipeline>()
                .map_err(|_| AppError::InvalidInput("pipeline needs 2+ linked elements".into()))?;
            let bus = pipeline
                .bus()
                .ok_or_else(|| AppError::Internal("pipeline has no bus".into()))?;
            let stop_flag = Arc::new(AtomicBool::new(false));
            let watcher = {
                let (p, events, flag) = (pipeline.clone(), self.bus.clone(), stop_flag.clone());
                std::thread::Builder::new()
                    .name("gst-bus-watcher".into())
                    .spawn(move || watch(p, bus, events, flag))
                    .map_err(|e| AppError::Internal(format!("watcher thread: {e}")))?
            };
            let started = pipeline.set_state(gst::State::Playing);
            self.running = Some(Running {
                pipeline,
                stop_flag,
                watcher: Some(watcher),
            });
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
            let Some(mut r) = self.running.take() else {
                return Ok(());
            };
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
    }

    impl Drop for GstBackend {
        fn drop(&mut self) {
            let _ = self.stop();
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
        let backend = GstBackend::new(Arc::clone(&bus)).expect("gstreamer initialises");
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
