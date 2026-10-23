//! The real streaming engine (feature `gstreamer`): one encode pipeline, a `tee` per elementary stream,
//! and one `queue ! flvmux ! rtmp2sink` bin per destination that can be added and removed while running.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gstreamer as gst;
use gstreamer::prelude::*;

use crate::error::AppError;
use crate::stream::{self, EncoderChoice, StreamSettings};
use crate::stream_service::{BackendEvent, StartPlan, StreamBackend};

const STATS_EVERY: Duration = Duration::from_secs(2);
/// Data reaching the sink proves nothing until the server has had time to refuse it; "Live" waits this long.
const CONFIRM_AFTER: Duration = Duration::from_millis(1500);
const BRANCH_QUEUE_NS: u64 = 3_000_000_000;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn internal(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("{what}: {e}"))
}

fn has_element(name: &str) -> bool {
    gst::ElementFactory::find(name).is_some()
}

/// Runs a few frames through `encoder` to prove it works on this machine (a listed NVENC can still fail without a GPU).
pub fn probe_encoder(choice: &EncoderChoice, settings: &StreamSettings) -> bool {
    let probe_settings = StreamSettings {
        width: 640,
        height: 360,
        fps: 30,
        ..settings.clone()
    };
    let description = format!(
        "videotestsrc num-buffers=15 ! videoconvert ! video/x-raw,width=640,height=360,framerate=30/1 ! {} ! fakesink sync=false",
        stream::encoder_description(choice, &probe_settings)
    );
    let Ok(element) = gst::parse::launch(&description) else {
        return false;
    };
    let Ok(pipeline) = element.downcast::<gst::Pipeline>() else {
        return false;
    };
    let ok = pipeline.set_state(gst::State::Playing).is_ok()
        && pipeline.bus().is_some_and(|bus| {
            bus.timed_pop_filtered(
                gst::ClockTime::from_seconds(5),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .is_some_and(|m| matches!(m.view(), gst::MessageView::Eos(_)))
        });
    let _ = pipeline.set_state(gst::State::Null);
    ok
}

/// Preferred encoder that is installed, licensed for use and actually works here.
pub fn choose_encoder(
    settings: &StreamSettings,
    allow_gpl: bool,
) -> Result<EncoderChoice, AppError> {
    let mut tried: Vec<&'static str> = Vec::new();
    let mut remaining_forced = settings.encoder.clone();
    loop {
        let skip = tried.clone();
        let available = move |e: &str| has_element(e) && !skip.contains(&e);
        let choice = stream::select_encoder(&available, &remaining_forced, allow_gpl)?;
        if probe_encoder(&choice, settings) {
            return Ok(choice);
        }
        tried.push(choice.element);
        if !remaining_forced.is_empty() {
            return Err(AppError::InvalidInput(format!(
                "encoder '{}' failed its test encode",
                choice.element
            )));
        }
        remaining_forced.clear();
    }
}

fn aac_encoder(kbps: u32) -> Result<String, AppError> {
    for name in ["avenc_aac", "voaacenc", "mfaacenc", "fdkaacenc"] {
        if has_element(name) {
            return Ok(format!("{name} bitrate={}", kbps * 1000));
        }
    }
    Err(AppError::Internal("no AAC encoder found".into()))
}

struct Branch {
    bin: gst::Bin,
    video_pad: gst::Pad,
    audio_pad: gst::Pad,
    counters: Arc<Counters>,
    last_stats: Instant,
    last_bytes: u64,
}

#[derive(Default)]
struct Counters {
    bytes: AtomicU64,
    drops: AtomicU64,
    first_buffer: Mutex<Option<Instant>>,
    announced: AtomicBool,
}

struct Running {
    pipeline: gst::Pipeline,
    vtee: gst::Element,
    atee: gst::Element,
    stop_bus: Arc<AtomicBool>,
    bus_thread: Option<JoinHandle<()>>,
}

pub struct GstStreamBackend {
    running: Option<Running>,
    branches: HashMap<String, Branch>,
    events: Arc<Mutex<Vec<BackendEvent>>>,
}

impl GstStreamBackend {
    pub fn new() -> Result<Self, AppError> {
        gst::init().map_err(|e| internal("GStreamer init failed", e))?;
        for needed in ["flvmux", "rtmp2sink", "tee", "h264parse", "aacparse"] {
            if !has_element(needed) {
                return Err(AppError::Internal(format!(
                    "GStreamer plugin for '{needed}' is missing; streaming is unavailable"
                )));
            }
        }
        Ok(Self {
            running: None,
            branches: HashMap::new(),
            events: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn push(events: &Mutex<Vec<BackendEvent>>, e: BackendEvent) {
        lock(events).push(e);
    }
}

/// A tee passes one branch's flow error up to the encoder, which then stops for every destination. Pushing each
/// buffer to the branch by hand (and ignoring its result) keeps one dead RTMP server from taking the others down.
fn isolate(tee_src: &gst::Pad) {
    tee_src.add_probe(
        gst::PadProbeType::BUFFER | gst::PadProbeType::BUFFER_LIST,
        |pad, info| {
            if let Some(peer) = pad.peer() {
                match info.data.take() {
                    Some(gst::PadProbeData::Buffer(b)) => {
                        let _ = peer.chain(b);
                    }
                    Some(gst::PadProbeData::BufferList(l)) => {
                        let _ = peer.chain_list(l);
                    }
                    other => info.data = other,
                }
            }
            gst::PadProbeReturn::Handled
        },
    );
}

fn force_keyframe(pad: &gst::Pad) {
    let s = gst::Structure::builder("GstForceKeyUnit")
        .field("timestamp", gst::ClockTime::NONE)
        .field("stream-time", gst::ClockTime::NONE)
        .field("running-time", gst::ClockTime::NONE)
        .field("all-headers", true)
        .field("count", 0u32)
        .build();
    pad.send_event(gst::event::CustomUpstream::new(s));
}

/// Which destination (if any) a bus message came from, by the bin's name.
fn destination_of(msg: &gst::Message) -> Option<String> {
    let path = msg.src()?.path_string();
    let tail = path.split("dest_").nth(1)?;
    Some(
        tail.split(['/', ':'])
            .next()
            .unwrap_or_default()
            .to_string(),
    )
}

fn spawn_bus_thread(
    pipeline: &gst::Pipeline,
    events: Arc<Mutex<Vec<BackendEvent>>>,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    let bus = pipeline.bus()?;
    Some(std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            let Some(msg) = bus.timed_pop_filtered(
                gst::ClockTime::from_mseconds(200),
                &[gst::MessageType::Error],
            ) else {
                continue;
            };
            if let Some(id) = destination_of(&msg) {
                // The text can contain the RTMP URL (and so the key), so only the fact is forwarded.
                GstStreamBackend::push(
                    &events,
                    BackendEvent::DestinationDown {
                        id,
                        reason: "connection error".into(),
                    },
                );
            }
        }
    }))
}

impl StreamBackend for GstStreamBackend {
    fn start(&mut self, plan: &StartPlan) -> Result<EncoderChoice, AppError> {
        if self.running.is_some() {
            return Err(AppError::InvalidInput(
                "the stream engine is already running".into(),
            ));
        }
        let choice = choose_encoder(&plan.settings, plan.allow_gpl_encoder)?;
        let s = &plan.settings;
        let description = format!(
            "{vhead} ! {venc} ! h264parse config-interval=-1 ! video/x-h264,stream-format=avc,alignment=au ! tee name=vtee allow-not-linked=true \
             {ahead} ! {aenc} ! aacparse ! audio/mpeg,mpegversion=4,stream-format=raw ! tee name=atee allow-not-linked=true",
            vhead = stream::video_head(&plan.video, s),
            venc = stream::encoder_description(&choice, s),
            ahead = stream::audio_head(&plan.audio),
            aenc = aac_encoder(s.audio_kbps)?,
        );
        let pipeline = gst::parse::launch(&description)
            .map_err(|e| internal("build stream pipeline", e))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| AppError::Internal("stream pipeline is not a pipeline".into()))?;
        let vtee = pipeline
            .by_name("vtee")
            .ok_or_else(|| AppError::Internal("video tee missing".into()))?;
        let atee = pipeline
            .by_name("atee")
            .ok_or_else(|| AppError::Internal("audio tee missing".into()))?;
        let stop_bus = Arc::new(AtomicBool::new(false));
        let bus_thread =
            spawn_bus_thread(&pipeline, Arc::clone(&self.events), Arc::clone(&stop_bus));
        if let Err(e) = pipeline.set_state(gst::State::Playing) {
            stop_bus.store(true, Ordering::SeqCst);
            let _ = pipeline.set_state(gst::State::Null);
            return Err(internal("start stream pipeline", e));
        }
        self.running = Some(Running {
            pipeline,
            vtee,
            atee,
            stop_bus,
            bus_thread,
        });
        Ok(choice)
    }

    fn add_destination(&mut self, id: &str, url: &str) -> Result<(), AppError> {
        let running = self
            .running
            .as_ref()
            .ok_or_else(|| AppError::Internal("the stream engine is not running".into()))?;
        if self.branches.contains_key(id) {
            return Err(AppError::InvalidInput(format!(
                "destination '{id}' is already attached"
            )));
        }
        let make = |factory: &str, name: &str| {
            gst::ElementFactory::make(factory)
                .name(name)
                .build()
                .map_err(|e| internal(&format!("create {factory}"), e))
        };
        let bin = gst::Bin::builder().name(format!("dest_{id}")).build();
        let queue = |name: &str| -> Result<gst::Element, AppError> {
            let q = make("queue", name)?;
            q.set_property_from_str("leaky", "downstream");
            q.set_property("max-size-buffers", 0u32);
            q.set_property("max-size-bytes", 0u32);
            q.set_property("max-size-time", BRANCH_QUEUE_NS);
            Ok(q)
        };
        let (vq, aq) = (queue(&format!("{id}_vq"))?, queue(&format!("{id}_aq"))?);
        let mux = make("flvmux", &format!("{id}_mux"))?;
        mux.set_property("streamable", true);
        let sink = make("rtmp2sink", &format!("{id}_out"))?;
        sink.set_property("location", url);
        sink.set_property("async", false);
        bin.add_many([&vq, &aq, &mux, &sink])
            .map_err(|e| internal("assemble destination", e))?;
        let vmux = mux
            .request_pad_simple("video")
            .ok_or_else(|| AppError::Internal("flvmux has no video pad".into()))?;
        let amux = mux
            .request_pad_simple("audio")
            .ok_or_else(|| AppError::Internal("flvmux has no audio pad".into()))?;
        let linked = (|| -> Result<(), gst::glib::BoolError> {
            vq.static_pad("src")
                .ok_or_else(|| gst::glib::bool_error!("vq src"))?
                .link(&vmux)
                .map(|_| ())
                .map_err(|_| gst::glib::bool_error!("link video"))?;
            aq.static_pad("src")
                .ok_or_else(|| gst::glib::bool_error!("aq src"))?
                .link(&amux)
                .map(|_| ())
                .map_err(|_| gst::glib::bool_error!("link audio"))?;
            mux.link(&sink)?;
            Ok(())
        })();
        linked.map_err(|e| internal("link destination", e))?;
        for (name, queue_el) in [("vsink", &vq), ("asink", &aq)] {
            let target = queue_el
                .static_pad("sink")
                .ok_or_else(|| AppError::Internal("queue has no sink pad".into()))?;
            let ghost = gst::GhostPad::builder_with_target(&target)
                .map_err(|e| internal("ghost pad", e))?
                .name(name)
                .build();
            bin.add_pad(&ghost)
                .map_err(|e| internal("add ghost pad", e))?;
        }

        let counters = Arc::new(Counters::default());
        if let Some(sink_pad) = sink.static_pad("sink") {
            let c = Arc::clone(&counters);
            sink_pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
                if let Some(gst::PadProbeData::Buffer(b)) = &info.data {
                    c.bytes.fetch_add(b.size() as u64, Ordering::Relaxed);
                    lock(&c.first_buffer).get_or_insert_with(Instant::now);
                }
                gst::PadProbeReturn::Ok
            });
        }
        for q in [&vq, &aq] {
            let c = Arc::clone(&counters);
            q.connect("overrun", false, move |_| {
                c.drops.fetch_add(1, Ordering::Relaxed);
                None
            });
        }

        running
            .pipeline
            .add(&bin)
            .map_err(|e| internal("add destination", e))?;
        let attach =
            |tee: &gst::Element, pad_name: &str| -> Result<(gst::Pad, gst::Pad), AppError> {
                let tee_pad = tee
                    .request_pad_simple("src_%u")
                    .ok_or_else(|| AppError::Internal("tee refused a new branch".into()))?;
                let target = bin
                    .static_pad(pad_name)
                    .ok_or_else(|| AppError::Internal("destination pad missing".into()))?;
                tee_pad
                    .link(&target)
                    .map_err(|e| internal("link tee", format!("{e:?}")))?;
                isolate(&tee_pad);
                Ok((tee_pad, target))
            };
        let (video_pad, _) = attach(&running.vtee, "vsink")?;
        let (audio_pad, _) = attach(&running.atee, "asink")?;
        bin.sync_state_with_parent()
            .map_err(|e| internal("start destination", e))?;
        force_keyframe(&video_pad);
        self.branches.insert(
            id.to_string(),
            Branch {
                bin,
                video_pad,
                audio_pad,
                counters,
                last_stats: Instant::now(),
                last_bytes: 0,
            },
        );
        Ok(())
    }

    fn remove_destination(&mut self, id: &str) -> Result<(), AppError> {
        let Some(branch) = self.branches.remove(id) else {
            return Ok(());
        };
        let Some(running) = self.running.as_ref() else {
            return Ok(());
        };
        for (tee, pad) in [
            (&running.vtee, &branch.video_pad),
            (&running.atee, &branch.audio_pad),
        ] {
            let (tx, rx) = mpsc::channel();
            pad.add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, move |pad, _| {
                if let Some(peer) = pad.peer() {
                    let _ = pad.unlink(&peer);
                }
                let _ = tx.send(());
                gst::PadProbeReturn::Drop
            });
            if rx.recv_timeout(Duration::from_secs(1)).is_err() {
                if let Some(peer) = pad.peer() {
                    let _ = pad.unlink(&peer);
                }
            }
            tee.release_request_pad(pad);
        }
        let _ = branch.bin.set_state(gst::State::Null);
        let _ = running.pipeline.remove(&branch.bin);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AppError> {
        let Some(mut running) = self.running.take() else {
            return Ok(());
        };
        self.branches.clear();
        let bus = running.pipeline.bus();
        if let Some(bus) = &bus {
            // EOS flows through flvmux so each server sees a clean end of stream.
            running.pipeline.send_event(gst::event::Eos::new());
            let _ =
                bus.timed_pop_filtered(gst::ClockTime::from_seconds(3), &[gst::MessageType::Eos]);
        }
        running.stop_bus.store(true, Ordering::SeqCst);
        if let Some(t) = running.bus_thread.take() {
            let _ = t.join();
        }
        let result = running.pipeline.set_state(gst::State::Null);
        lock(&self.events).clear();
        result
            .map(|_| ())
            .map_err(|e| internal("stop stream pipeline", e))
    }

    fn poll(&mut self) -> Vec<BackendEvent> {
        let mut out = std::mem::take(&mut *lock(&self.events));
        let now = Instant::now();
        for (id, b) in &mut self.branches {
            let confirmed = lock(&b.counters.first_buffer)
                .is_some_and(|t| now.duration_since(t) >= CONFIRM_AFTER);
            if confirmed && !b.counters.announced.swap(true, Ordering::SeqCst) {
                out.push(BackendEvent::DestinationUp(id.clone()));
            }
            let elapsed = now.duration_since(b.last_stats);
            if elapsed >= STATS_EVERY && b.counters.announced.load(Ordering::SeqCst) {
                let bytes = b.counters.bytes.load(Ordering::Relaxed);
                let kbps = ((bytes - b.last_bytes) * 8 / 1000 / elapsed.as_secs().max(1)) as u32;
                b.last_bytes = bytes;
                b.last_stats = now;
                out.push(BackendEvent::Stats {
                    id: id.clone(),
                    bitrate_kbps: kbps,
                    dropped_frames: b.counters.drops.load(Ordering::Relaxed),
                });
            }
        }
        out
    }
}

impl Drop for GstStreamBackend {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
