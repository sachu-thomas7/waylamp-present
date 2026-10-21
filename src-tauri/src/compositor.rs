use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::config::StreamDeckConfig;
use crate::error::AppError;

pub const OUTPUT_WIDTH: u32 = 1920;
pub const OUTPUT_HEIGHT: u32 = 1080;
pub const OUTPUT_FPS: u32 = 30;
pub const OUTPUT_CAPS: &str = "video/x-raw,width=1920,height=1080,framerate=30/1";
pub const MAX_SOURCES: usize = 8;
const MAX_DIMENSION: u32 = 7680;
const MAX_OFFSET: i32 = 16384;
const MAX_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// Live `videotestsrc` standing in for a camera.
    #[default]
    Test,
    Ndi,
}

/// Where one source sits on the 1920x1080 canvas; a higher `z` is drawn on top.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SourceGeometry {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub z: u32,
    pub kind: SourceKind,
}

/// One `[scene.<id>]` table; F3.3 adds overlays and transitions beside `sources`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneConfig {
    pub sources: Vec<SourceGeometry>,
}

/// What the backend reports once the compositor is up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositorInfo {
    pub element: String,
    pub hardware: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompositorStatus {
    pub running: bool,
    pub element: Option<String>,
    pub hardware: bool,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub sources: Vec<SourceGeometry>,
}

impl Default for CompositorStatus {
    fn default() -> Self {
        Self {
            running: false,
            element: None,
            hardware: false,
            width: OUTPUT_WIDTH,
            height: OUTPUT_HEIGHT,
            fps: OUTPUT_FPS,
            sources: Vec::new(),
        }
    }
}

/// Sink-pad properties of one source on the compositor element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadProps {
    pub xpos: i32,
    pub ypos: i32,
    pub width: i32,
    pub height: i32,
    pub zorder: u32,
}

pub fn pad_properties(g: &SourceGeometry) -> PadProps {
    PadProps {
        xpos: g.x,
        ypos: g.y,
        width: g.w as i32,
        height: g.h as i32,
        zorder: g.z,
    }
}

pub fn validate_layout(layout: &[SourceGeometry]) -> Result<(), AppError> {
    let bad = |m: String| Err(AppError::InvalidInput(m));
    if layout.len() > MAX_SOURCES {
        return bad(format!(
            "at most {MAX_SOURCES} sources, got {}",
            layout.len()
        ));
    }
    let mut seen = BTreeSet::new();
    for g in layout {
        let n = &g.name;
        if n.trim().is_empty() || n.len() > MAX_NAME_LEN || n.chars().any(char::is_control) {
            return bad(format!("invalid source name {n:?}"));
        }
        if !seen.insert(n.as_str()) {
            return bad(format!("duplicate source name: {n}"));
        }
        if g.w == 0 || g.h == 0 || g.w > MAX_DIMENSION || g.h > MAX_DIMENSION {
            return bad(format!("{n}: size {}x{} out of range", g.w, g.h));
        }
        if g.x.abs() > MAX_OFFSET || g.y.abs() > MAX_OFFSET {
            return bad(format!("{n}: position {},{} out of range", g.x, g.y));
        }
        let (right, bottom) = (g.x as i64 + g.w as i64, g.y as i64 + g.h as i64);
        if right <= 0 || bottom <= 0 || g.x >= OUTPUT_WIDTH as i32 || g.y >= OUTPUT_HEIGHT as i32 {
            return bad(format!("{n}: entirely outside the 1920x1080 canvas"));
        }
    }
    Ok(())
}

/// Picture-in-picture layout of three test sources, used when no scene id is given.
pub fn demo_layout() -> Vec<SourceGeometry> {
    let src = |name: &str, x, y, w, h, z| SourceGeometry {
        name: name.into(),
        x,
        y,
        w,
        h,
        z,
        kind: SourceKind::Test,
    };
    vec![
        src("cam-main", 0, 0, 1920, 1080, 0),
        src("cam-2", 1280, 720, 640, 360, 1),
        src("cam-3", 0, 720, 640, 360, 1),
    ]
}

pub fn list_scenes(config: &StreamDeckConfig) -> Vec<String> {
    config.scene.keys().cloned().collect()
}

/// The validated layout of scene `scene_id`, or the demo layout when `None`.
pub fn resolve_layout(
    config: &StreamDeckConfig,
    scene_id: Option<&str>,
) -> Result<Vec<SourceGeometry>, AppError> {
    let Some(id) = scene_id else {
        return Ok(demo_layout());
    };
    let scene = config
        .scene
        .get(id)
        .ok_or_else(|| AppError::NotFound(format!("scene not found: {id}")))?;
    validate_layout(&scene.sources)
        .map_err(|e| AppError::InvalidInput(format!("scene {id}: {e}")))?;
    Ok(scene.sources.clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
    Other,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositorChoice {
    pub element: &'static str,
    pub hardware: bool,
    /// Elements that must follow the compositor to hand system-memory frames to the capsfilter.
    pub post: &'static [&'static str],
}

const CPU: CompositorChoice = CompositorChoice {
    element: "compositor",
    hardware: false,
    post: &[],
};

/// GPU variant per platform when it and its helper elements exist, else the CPU `compositor`.
pub fn select_compositor(platform: Platform, available: impl Fn(&str) -> bool) -> CompositorChoice {
    let gpu = match platform {
        Platform::Windows => Some(CompositorChoice {
            element: "d3d12compositor",
            hardware: true,
            post: &[],
        }),
        Platform::MacOs | Platform::Linux => Some(CompositorChoice {
            element: "glvideomixer",
            hardware: true,
            post: &["gldownload"],
        }),
        Platform::Other => None,
    };
    match gpu {
        Some(c) if available(c.element) && c.post.iter().all(|p| available(p)) => c,
        _ => CPU,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSink {
    Display,
    /// Paced like a real sink but renders nothing (zero processing deadline, else its 20 ms is added to the compositor wait); for tests and headless runs.
    Discard,
}

/// Static part of the pipeline; the mixer is `mix`, the enforced output caps are `out_caps`.
pub fn spine_description(choice: &CompositorChoice, sink: OutputSink) -> String {
    let post: String = choice.post.iter().map(|p| format!("{p} ! ")).collect();
    let tail = match sink {
        OutputSink::Display => "videoconvert ! autovideosink",
        OutputSink::Discard => "fakesink name=out_sink sync=true processing-deadline=0",
    };
    format!(
        "{} name=mix ! {post}capsfilter name=out_caps caps=\"{OUTPUT_CAPS}\" ! {tail}",
        choice.element
    )
}

/// Input branch (NV12: I420 uploads ran d3d12compositor at ~23 fps with 3 sources); its unlinked pad becomes the `src` ghost pad.
pub fn source_branch_description(g: &SourceGeometry, index: usize) -> String {
    const PATTERNS: [&str; 4] = ["smpte", "ball", "pinwheel", "checkers-8"];
    match g.kind {
        SourceKind::Test => format!(
            "videotestsrc is-live=true pattern={} ! video/x-raw,width={OUTPUT_WIDTH},height={OUTPUT_HEIGHT},framerate={OUTPUT_FPS}/1,format=NV12 ! queue max-size-buffers=3 leaky=downstream",
            PATTERNS[index % PATTERNS.len()]
        ),
        SourceKind::Ndi => {
            let escaped = g.name.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                "ndisrc ndi-name=\"{escaped}\" ! queue ! videoconvert ! video/x-raw,format=NV12 ! queue max-size-buffers=3 leaky=downstream"
            )
        }
    }
}

#[cfg(feature = "gstreamer")]
pub mod gst_compositor {
    use std::collections::HashMap;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    use gstreamer as gst;
    use gstreamer::prelude::*;

    use super::*;
    use crate::bus::EventBus;
    use crate::pipeline::gst_backend::{build_running, teardown, Running};

    pub fn factory_available(name: &str) -> bool {
        gst::ElementFactory::find(name).is_some()
    }

    struct Branch {
        bin: gst::Bin,
        mix_pad: gst::Pad,
        geometry: SourceGeometry,
    }

    /// One live compositor pipeline; sources join and leave while it runs.
    pub struct GstCompositor {
        running: Running,
        mix: gst::Element,
        branches: HashMap<String, Branch>,
        counter: usize,
        pub info: CompositorInfo,
    }

    fn internal(what: &str, e: impl std::fmt::Display) -> AppError {
        AppError::Internal(format!("{what}: {e}"))
    }

    fn apply_pad(pad: &gst::Pad, p: &PadProps) -> Result<(), AppError> {
        let missing = |n: &str| AppError::Internal(format!("compositor pad has no {n} property"));
        for (name, value) in [
            ("xpos", p.xpos),
            ("ypos", p.ypos),
            ("width", p.width),
            ("height", p.height),
        ] {
            pad.find_property(name).ok_or_else(|| missing(name))?;
            pad.set_property(name, value);
        }
        pad.find_property("zorder")
            .ok_or_else(|| missing("zorder"))?;
        pad.set_property("zorder", p.zorder);
        Ok(())
    }

    impl GstCompositor {
        pub fn start(
            events: Arc<EventBus>,
            layout: &[SourceGeometry],
            sink: OutputSink,
            available: &dyn Fn(&str) -> bool,
        ) -> Result<Self, AppError> {
            let choice = select_compositor(Platform::current(), available);
            let running = build_running(&spine_description(&choice, sink), events)?;
            let mix = running
                .pipeline
                .by_name("mix")
                .ok_or_else(|| AppError::Internal("compositor element missing".into()))?;
            if mix.find_property("background").is_some() {
                mix.set_property_from_str("background", "black");
            }
            let mut c = Self {
                running,
                mix,
                branches: HashMap::new(),
                counter: 0,
                info: CompositorInfo {
                    element: choice.element.into(),
                    hardware: choice.hardware,
                },
            };
            for g in layout {
                if let Err(e) = c.add_branch(g) {
                    let _ = c.stop();
                    return Err(e);
                }
            }
            if let Err(e) = c.running.pipeline.set_state(gst::State::Playing) {
                let _ = c.stop();
                return Err(internal("compositor failed to start", e));
            }
            Ok(c)
        }

        pub fn pipeline(&self) -> &gst::Pipeline {
            &self.running.pipeline
        }

        /// The live sink-pad properties of `name`, read back from the element.
        pub fn pad_props(&self, name: &str) -> Option<PadProps> {
            let pad = &self.branches.get(name)?.mix_pad;
            Some(PadProps {
                xpos: pad.property("xpos"),
                ypos: pad.property("ypos"),
                width: pad.property("width"),
                height: pad.property("height"),
                zorder: pad.property("zorder"),
            })
        }

        pub fn source_names(&self) -> Vec<String> {
            let mut names: Vec<_> = self.branches.keys().cloned().collect();
            names.sort();
            names
        }

        fn add_branch(&mut self, g: &SourceGeometry) -> Result<(), AppError> {
            let index = self.counter;
            self.counter += 1;
            let bin = gst::parse::bin_from_description(&source_branch_description(g, index), true)
                .map_err(|e| AppError::InvalidInput(format!("invalid source {}: {e}", g.name)))?;
            bin.set_property("name", format!("source-{index}"));
            let pipeline = &self.running.pipeline;
            pipeline.add(&bin).map_err(|e| internal("add source", e))?;
            let linked = (|| {
                let src = bin
                    .static_pad("src")
                    .ok_or_else(|| AppError::Internal("source has no output pad".into()))?;
                let pad = self
                    .mix
                    .request_pad_simple("sink_%u")
                    .ok_or_else(|| AppError::Internal("compositor refused a new input".into()))?;
                let wired = apply_pad(&pad, &pad_properties(g)).and_then(|_| {
                    src.link(&pad)
                        .map(|_| ())
                        .map_err(|e| internal("link source", e))
                });
                match wired {
                    Ok(()) => Ok(pad),
                    Err(e) => {
                        self.mix.release_request_pad(&pad);
                        Err(e)
                    }
                }
            })();
            let mix_pad = match linked {
                Ok(p) => p,
                Err(e) => {
                    let _ = bin.set_state(gst::State::Null);
                    let _ = pipeline.remove(&bin);
                    return Err(e);
                }
            };
            if let Err(e) = bin.sync_state_with_parent() {
                self.detach(&bin, &mix_pad);
                return Err(internal("start source", e));
            }
            self.branches.insert(
                g.name.clone(),
                Branch {
                    bin,
                    mix_pad,
                    geometry: g.clone(),
                },
            );
            Ok(())
        }

        /// Blocks the source's output, unlinks it, then frees the compositor pad so no half frame leaks.
        fn detach(&self, bin: &gst::Bin, mix_pad: &gst::Pad) {
            if let Some(src) = bin.static_pad("src") {
                let (tx, rx) = mpsc::channel();
                let target = mix_pad.clone();
                src.add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, move |pad, _| {
                    let _ = pad.unlink(&target);
                    let _ = tx.send(());
                    gst::PadProbeReturn::Drop
                });
                if rx.recv_timeout(Duration::from_secs(1)).is_err() {
                    let _ = src.unlink(mix_pad);
                }
            }
            self.mix.release_request_pad(mix_pad);
            let _ = bin.set_state(gst::State::Null);
            let _ = self.running.pipeline.remove(bin);
        }

        fn remove_branch(&mut self, name: &str) {
            if let Some(b) = self.branches.remove(name) {
                self.detach(&b.bin, &b.mix_pad);
            }
        }

        /// Adds new sources, moves existing ones live, then removes the ones no longer wanted.
        pub fn set_layout(&mut self, layout: &[SourceGeometry]) -> Result<(), AppError> {
            for g in layout {
                if self
                    .branches
                    .get(&g.name)
                    .is_some_and(|b| b.geometry.kind != g.kind)
                {
                    self.remove_branch(&g.name);
                }
                match self.branches.get_mut(&g.name) {
                    Some(b) => {
                        apply_pad(&b.mix_pad, &pad_properties(g))?;
                        b.geometry = g.clone();
                    }
                    None => self.add_branch(g)?,
                }
            }
            let keep: BTreeSet<&str> = layout.iter().map(|g| g.name.as_str()).collect();
            let gone: Vec<String> = self
                .branches
                .keys()
                .filter(|n| !keep.contains(n.as_str()))
                .cloned()
                .collect();
            gone.iter().for_each(|n| self.remove_branch(n));
            Ok(())
        }

        pub fn stop(self) -> Result<(), AppError> {
            teardown(self.running)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(name: &str, x: i32, y: i32, w: u32, h: u32, z: u32) -> SourceGeometry {
        SourceGeometry {
            name: name.into(),
            x,
            y,
            w,
            h,
            z,
            kind: SourceKind::Test,
        }
    }

    fn only(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |n| names.contains(&n)
    }

    #[test]
    fn windows_prefers_d3d12_and_falls_back_to_cpu() {
        let all = only(&["d3d12compositor", "compositor"]);
        let c = select_compositor(Platform::Windows, all);
        assert_eq!((c.element, c.hardware), ("d3d12compositor", true));
        let c = select_compositor(Platform::Windows, only(&["compositor"]));
        assert_eq!((c.element, c.hardware), ("compositor", false));
    }

    #[test]
    fn macos_and_linux_prefer_glvideomixer_with_a_download_stage() {
        for p in [Platform::MacOs, Platform::Linux] {
            let c = select_compositor(p, only(&["glvideomixer", "gldownload", "compositor"]));
            assert_eq!(
                (c.element, c.hardware, c.post),
                ("glvideomixer", true, &["gldownload"][..])
            );
            // The GPU mixer is useless without the download stage, so it is not chosen alone.
            let c = select_compositor(p, only(&["glvideomixer", "compositor"]));
            assert_eq!(c.element, "compositor");
        }
    }

    #[test]
    fn platforms_never_pick_another_platforms_gpu_element() {
        let everything = |_: &str| true;
        assert_eq!(
            select_compositor(Platform::Windows, everything).element,
            "d3d12compositor"
        );
        assert_eq!(
            select_compositor(Platform::Linux, everything).element,
            "glvideomixer"
        );
        assert_eq!(
            select_compositor(Platform::Other, everything).element,
            "compositor"
        );
        assert_eq!(
            select_compositor(Platform::Windows, |_| false).element,
            "compositor"
        );
    }

    #[test]
    fn geometry_maps_to_sink_pad_properties() {
        let p = pad_properties(&geo("a", -40, 720, 640, 360, 3));
        assert_eq!(
            p,
            PadProps {
                xpos: -40,
                ypos: 720,
                width: 640,
                height: 360,
                zorder: 3
            }
        );
    }

    #[test]
    fn output_caps_are_1080p30() {
        assert_eq!(
            OUTPUT_CAPS,
            format!(
                "video/x-raw,width={OUTPUT_WIDTH},height={OUTPUT_HEIGHT},framerate={OUTPUT_FPS}/1"
            )
        );
        let d = spine_description(&CPU, OutputSink::Discard);
        assert_eq!(
            d,
            "compositor name=mix ! capsfilter name=out_caps caps=\"video/x-raw,width=1920,height=1080,framerate=30/1\" ! fakesink name=out_sink sync=true processing-deadline=0"
        );
    }

    #[test]
    fn spine_inserts_the_download_stage_between_mixer_and_caps() {
        let gl = select_compositor(Platform::Linux, |_| true);
        let d = spine_description(&gl, OutputSink::Display);
        assert!(d.starts_with("glvideomixer name=mix ! gldownload ! capsfilter name=out_caps"));
        assert!(d.ends_with("videoconvert ! autovideosink"));
    }

    #[test]
    fn ndi_branch_escapes_the_source_name() {
        let mut g = geo(r#"CAM "1" (Mevo\Core)"#, 0, 0, 640, 360, 0);
        g.kind = SourceKind::Ndi;
        let d = source_branch_description(&g, 0);
        assert!(
            d.starts_with(r#"ndisrc ndi-name="CAM \"1\" (Mevo\\Core)" ! "#),
            "{d}"
        );
    }

    #[test]
    fn test_branches_cycle_patterns_and_run_live_at_1080p30() {
        let a = source_branch_description(&geo("a", 0, 0, 1, 1, 0), 0);
        let b = source_branch_description(&geo("b", 0, 0, 1, 1, 0), 1);
        assert_ne!(a, b);
        assert!(
            a.contains("is-live=true")
                && a.contains("width=1920,height=1080,framerate=30/1,format=NV12")
        );
    }

    #[test]
    fn valid_layouts_pass_including_partly_off_canvas() {
        validate_layout(&demo_layout()).expect("demo layout");
        validate_layout(&[geo("a", -100, -100, 400, 400, 0)]).expect("partly off canvas");
        validate_layout(&[]).expect("empty is a valid layout");
    }

    #[test]
    fn invalid_layouts_are_rejected_with_the_offending_source() {
        let cases = [
            (vec![geo("", 0, 0, 10, 10, 0)], "invalid source name"),
            (
                vec![geo("a", 0, 0, 10, 10, 0), geo("a", 5, 5, 10, 10, 1)],
                "duplicate",
            ),
            (vec![geo("a", 0, 0, 0, 10, 0)], "size"),
            (vec![geo("a", 0, 0, 10, 99999, 0)], "size"),
            (vec![geo("a", 1920, 0, 10, 10, 0)], "outside"),
            (vec![geo("a", -50, 0, 50, 10, 0)], "outside"),
            (vec![geo("a", 999_999, 0, 10, 10, 0)], "position"),
            (
                (0..9)
                    .map(|i| geo(&format!("s{i}"), 0, 0, 10, 10, 0))
                    .collect(),
                "at most",
            ),
        ];
        for (layout, needle) in cases {
            let err = validate_layout(&layout).expect_err(needle);
            assert!(
                matches!(&err, AppError::InvalidInput(m) if m.contains(needle)),
                "{err:?}"
            );
        }
    }

    #[test]
    fn scene_sources_parse_from_toml_with_defaults() {
        let cfg: StreamDeckConfig = toml::from_str(
            r#"
[[scene.keynote.sources]]
name = "Cam 1"
w = 1920
h = 1080

[[scene.keynote.sources]]
name = "Cam 2"
kind = "ndi"
x = 1280
y = 720
w = 640
h = 360
z = 2
"#,
        )
        .expect("parses");
        let s = &cfg.scene["keynote"].sources;
        assert_eq!(s[0], geo("Cam 1", 0, 0, 1920, 1080, 0));
        assert_eq!((s[1].kind, s[1].z, s[1].x), (SourceKind::Ndi, 2, 1280));
        assert_eq!(list_scenes(&cfg), ["keynote"]);
    }

    #[test]
    fn configs_without_scenes_still_load_and_scenes_round_trip() {
        let old: StreamDeckConfig = toml::from_str("version = 1\n").expect("old config");
        assert!(old.scene.is_empty());
        let mut cfg = StreamDeckConfig::default();
        cfg.scene.insert(
            "a".into(),
            SceneConfig {
                sources: demo_layout(),
            },
        );
        let back: StreamDeckConfig =
            toml::from_str(&toml::to_string(&cfg).expect("ser")).expect("de");
        assert_eq!(back.scene["a"].sources, demo_layout());
    }

    #[test]
    fn resolve_layout_handles_demo_missing_and_invalid_scenes() {
        let mut cfg = StreamDeckConfig::default();
        cfg.scene.insert(
            "ok".into(),
            SceneConfig {
                sources: demo_layout(),
            },
        );
        cfg.scene.insert(
            "bad".into(),
            SceneConfig {
                sources: vec![geo("a", 0, 0, 0, 0, 0)],
            },
        );
        assert_eq!(resolve_layout(&cfg, None).expect("demo"), demo_layout());
        assert_eq!(resolve_layout(&cfg, Some("ok")).expect("ok").len(), 3);
        assert!(matches!(
            resolve_layout(&cfg, Some("nope")),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            resolve_layout(&cfg, Some("bad")),
            Err(AppError::InvalidInput(m)) if m.starts_with("scene bad:")
        ));
    }
}

#[cfg(all(test, feature = "gstreamer"))]
mod gst_compositor_tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use gstreamer as gst;
    use gstreamer::prelude::*;

    use super::gst_compositor::{factory_available, GstCompositor};
    use super::*;
    use crate::bus::EventBus;
    use crate::events::AppEvent;
    use crate::pipeline::gst_backend::GstBackend;
    use crate::pipeline::PipelineBackend;
    use crate::preview::PreviewHub;

    type Stamps = Arc<Mutex<Vec<(Instant, u64)>>>;
    type Filter = fn(&str) -> bool;

    fn cpu_only(n: &str) -> bool {
        n == "compositor"
    }

    fn sleep(ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn backend(filter: Option<Filter>) -> (GstBackend, Arc<EventBus>) {
        let bus = Arc::new(EventBus::new().0);
        let b = GstBackend::new(bus.clone(), PreviewHub::new())
            .expect("gstreamer initialises")
            .with_compositor_sink(OutputSink::Discard);
        let b = match filter {
            Some(f) => b.with_element_filter(f),
            None => b,
        };
        (b, bus)
    }

    fn stamp_pad(pad: &gst::Pad) -> Stamps {
        let stamps: Stamps = Default::default();
        let sink = stamps.clone();
        pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
            if let Some(gst::PadProbeData::Buffer(b)) = &info.data {
                if let Ok(mut v) = sink.lock() {
                    v.push((Instant::now(), b.pts().map(|t| t.nseconds()).unwrap_or(0)));
                }
            }
            gst::PadProbeReturn::Ok
        });
        stamps
    }

    fn out_pad(c: &GstCompositor) -> gst::Pad {
        c.pipeline()
            .by_name("out_caps")
            .and_then(|e| e.static_pad("src"))
            .expect("out_caps src pad")
    }

    fn out_caps(c: &GstCompositor) -> (i32, i32, i32, i32) {
        let caps = out_pad(c).current_caps().expect("negotiated output caps");
        let s = caps.structure(0).expect("structure");
        let fps = s.get::<gst::Fraction>("framerate").expect("framerate");
        (
            s.get("width").expect("width"),
            s.get("height").expect("height"),
            fps.numer(),
            fps.denom(),
        )
    }

    fn since(stamps: &Stamps, t: Instant) -> Vec<Instant> {
        let v = stamps.lock().expect("stamps");
        v.iter().map(|(i, _)| *i).filter(|i| *i >= t).collect()
    }

    fn max_gap(times: &[Instant]) -> Duration {
        times
            .windows(2)
            .map(|w| w[1] - w[0])
            .max()
            .unwrap_or_default()
    }

    fn errors(rx: &mut tokio::sync::broadcast::Receiver<AppEvent>) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            if let AppEvent::PipelineError { element, message } = e {
                out.push(format!("{element}: {message}"));
            }
        }
        out
    }

    /// The first `n` of the demo sources followed by extra top-right thumbnails.
    fn layout(n: usize) -> Vec<SourceGeometry> {
        let extra = (3..n).map(|i| SourceGeometry {
            name: format!("cam-{}", i + 1),
            x: 1280,
            y: 0,
            w: 640,
            h: 360,
            z: 1,
            kind: SourceKind::Test,
        });
        demo_layout().into_iter().chain(extra).take(n).collect()
    }

    #[test]
    fn three_sources_give_a_stable_1080p30_output() {
        let (mut b, bus) = backend(None);
        let mut rx = bus.subscribe();
        let info = b.compositor_start(&demo_layout()).expect("start");
        let c = b.compositor().expect("running");
        let stamps = stamp_pad(&out_pad(c));
        sleep(1000);
        let t = Instant::now();
        sleep(3000);
        let times = since(&stamps, t);
        assert_eq!(out_caps(c), (1920, 1080, 30, 1));
        let fps = times.len() as f64 / 3.0;
        println!(
            "element={} frames={} fps={fps:.1} max_gap={:?}",
            info.element,
            times.len(),
            max_gap(&times)
        );
        assert!((27.0..=33.0).contains(&fps), "fps {fps}");
        assert!(max_gap(&times) < Duration::from_millis(150));
        assert_eq!(errors(&mut rx), Vec::<String>::new());
        b.shutdown();
    }

    #[test]
    fn a_source_can_join_and_leave_without_interrupting_the_output() {
        let (mut b, bus) = backend(None);
        let mut rx = bus.subscribe();
        b.compositor_start(&layout(2)).expect("start");
        sleep(800);
        let t = Instant::now();
        let pad = out_pad(b.compositor().expect("running"));
        let stamps = stamp_pad(&pad);
        let caps_events = Arc::new(Mutex::new(0u32));
        let n = caps_events.clone();
        pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |_, info| {
            if let Some(gst::PadProbeData::Event(e)) = &info.data {
                if matches!(e.view(), gst::EventView::Caps(_)) {
                    *n.lock().expect("n") += 1;
                }
            }
            gst::PadProbeReturn::Ok
        });
        for step in [layout(3), layout(4), layout(2), layout(3)] {
            b.compositor_set_layout(&step).expect("set_layout");
            let c = b.compositor().expect("running");
            assert_eq!(c.source_names().len(), step.len());
            sleep(700);
        }
        let times = since(&stamps, t);
        let gap = max_gap(&times);
        println!("frames={} max_gap_across_add_remove={gap:?}", times.len());
        assert!(
            gap < Duration::from_millis(150),
            "output stalled for {gap:?}"
        );
        let c = b.compositor().expect("running");
        assert_eq!(out_caps(c), (1920, 1080, 30, 1));
        assert_eq!(
            *caps_events.lock().expect("n"),
            0,
            "output caps renegotiated"
        );
        assert_eq!(errors(&mut rx), Vec::<String>::new());
        b.shutdown();
    }

    #[test]
    fn layout_updates_move_sources_live_and_swap_z_order() {
        let (mut b, _bus) = backend(None);
        b.compositor_start(&layout(2)).expect("start");
        let mut moved = layout(2);
        moved[1].x = 100;
        moved[1].y = 50;
        moved[1].z = 7;
        b.compositor_set_layout(&moved).expect("move");
        let c = b.compositor().expect("running");
        assert_eq!(c.pad_props("cam-2"), Some(pad_properties(&moved[1])));
        assert_eq!(c.source_names(), ["cam-2", "cam-main"]);
        b.shutdown();
    }

    #[test]
    fn missing_gpu_elements_fall_back_to_the_cpu_compositor() {
        let (mut b, _bus) = backend(Some(cpu_only));
        let info = b.compositor_start(&layout(3)).expect("start");
        assert_eq!(
            (info.element.as_str(), info.hardware),
            ("compositor", false)
        );
        let stamps = stamp_pad(&out_pad(b.compositor().expect("running")));
        sleep(1500);
        assert!(
            stamps.lock().expect("stamps").len() > 20,
            "no CPU compositor output"
        );
        b.shutdown();
    }

    #[test]
    fn the_gpu_variant_is_selected_and_renders_when_installed() {
        let want = select_compositor(Platform::current(), factory_available);
        if !want.hardware {
            println!(
                "SKIPPED: no GPU compositor installed, would use {}",
                want.element
            );
            return;
        }
        let (mut b, bus) = backend(None);
        let mut rx = bus.subscribe();
        let info = b.compositor_start(&layout(3)).expect("start");
        assert_eq!((info.element.as_str(), info.hardware), (want.element, true));
        let stamps = stamp_pad(&out_pad(b.compositor().expect("running")));
        sleep(1500);
        assert!(
            stamps.lock().expect("stamps").len() > 20,
            "no output from {}",
            want.element
        );
        assert_eq!(
            out_caps(b.compositor().expect("running")),
            (1920, 1080, 30, 1)
        );
        assert_eq!(errors(&mut rx), Vec::<String>::new());
        b.shutdown();
    }

    #[test]
    fn start_twice_and_layout_without_start_are_rejected_and_stop_is_idempotent() {
        let (mut b, _bus) = backend(Some(cpu_only));
        assert!(matches!(
            b.compositor_set_layout(&layout(1)),
            Err(AppError::InvalidInput(_))
        ));
        b.compositor_start(&layout(1)).expect("start");
        assert!(matches!(
            b.compositor_start(&layout(1)),
            Err(AppError::InvalidInput(_))
        ));
        b.compositor_stop().expect("stop");
        b.compositor_stop().expect("stop again");
        b.compositor_start(&layout(1)).expect("restart");
        b.shutdown();
    }

    #[test]
    fn ndi_source_without_the_plugin_fails_cleanly_and_leaves_nothing_running() {
        if factory_available("ndisrc") {
            return;
        }
        let (mut b, _bus) = backend(Some(cpu_only));
        let mut l = layout(1);
        l[0].kind = SourceKind::Ndi;
        assert!(matches!(
            b.compositor_start(&l),
            Err(AppError::InvalidInput(_))
        ));
        assert!(b.compositor().is_none());
        b.compositor_start(&layout(1)).expect("still usable");
        b.shutdown();
    }

    const FRAME_NS: u64 = 33_333_334;

    fn percentile(v: &[f64], p: f64) -> f64 {
        let mut v = v.to_vec();
        v.sort_by(|a, b| a.total_cmp(b));
        v.get(((v.len() as f64 - 1.0) * p) as usize)
            .copied()
            .unwrap_or(f64::NAN)
    }

    /// Wall-clock ms from a frame reaching a compositor input to the composite frame carrying it; the frame is the one whose pts falls inside the output slot.
    fn input_to_output_ms(inputs: &Stamps, outputs: &Stamps) -> Vec<f64> {
        let ins = inputs.lock().expect("inputs").clone();
        let outs = outputs.lock().expect("outputs").clone();
        let mut ms = Vec::new();
        for (at_out, out_pts) in outs.iter().skip(30) {
            let in_slot = ins
                .iter()
                .find(|(_, pts)| *pts >= *out_pts && *pts - *out_pts < FRAME_NS);
            if let Some((at_in, _)) = in_slot {
                ms.push(at_out.saturating_duration_since(*at_in).as_secs_f64() * 1e3);
            }
        }
        ms
    }

    // Measurement, not a gate: cargo test --release --lib --features gstreamer latency_input -- --ignored --nocapture
    #[test]
    #[ignore = "measurement, not a gate"]
    fn latency_input_frame_to_composite_frame_with_three_sources() {
        for (label, filter, n) in [
            ("default", None, 3),
            ("cpu", Some(cpu_only as Filter), 3),
            ("cpu-1-source", Some(cpu_only as Filter), 1),
        ] {
            for run in 0..4 {
                let (mut b, _bus) = backend(filter);
                let info = b.compositor_start(&layout(n)).expect("start");
                let c = b.compositor().expect("running");
                let inputs: Vec<Stamps> = (0..n)
                    .map(|i| {
                        let pad = c
                            .pipeline()
                            .by_name(&format!("source-{i}"))
                            .and_then(|e| e.static_pad("src"))
                            .expect("source pad");
                        stamp_pad(&pad)
                    })
                    .collect();
                let at_output = stamp_pad(&out_pad(c));
                sleep(6000);
                let mut q = gst::query::Latency::new();
                let reported = c.pipeline().query(&mut q).then(|| q.result());
                let per_source: Vec<Vec<f64>> = inputs
                    .iter()
                    .map(|i| input_to_output_ms(i, &at_output))
                    .collect();
                let p50s: Vec<String> = per_source
                    .iter()
                    .map(|m| format!("{:.1}", percentile(m, 0.5)))
                    .collect();
                // The composite frame is only as fresh as its oldest input.
                let worst: Vec<f64> = (0..per_source[0].len())
                    .map(|k| {
                        per_source
                            .iter()
                            .filter_map(|m| m.get(k))
                            .fold(0.0, |a, b| f64::max(a, *b))
                    })
                    .collect();
                println!(
                    "[{label} run {run}] element={} per_input_p50={p50s:?} oldest_input_p50={:.1}ms p95={:.1}ms max={:.1}ms reported_pipeline_latency={reported:?}",
                    info.element,
                    percentile(&worst, 0.5),
                    percentile(&worst, 0.95),
                    percentile(&worst, 1.0),
                );
                b.shutdown();
            }
        }
    }

    /// Process CPU (percent of one core) over each of `windows` consecutive 4 s windows.
    fn process_cpu_samples(windows: usize) -> Vec<f64> {
        use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};
        let pid = get_current_pid().expect("pid");
        let mut sys = System::new();
        let kind = ProcessRefreshKind::nothing().with_cpu();
        let mut cpu_time = || {
            sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, kind);
            sys.process(pid)
                .map(|p| p.accumulated_cpu_time())
                .unwrap_or(0)
        };
        (0..windows)
            .map(|_| {
                let (before, t) = (cpu_time(), Instant::now());
                sleep(4000);
                (cpu_time() - before) as f64 / t.elapsed().as_millis() as f64 * 100.0
            })
            .collect()
    }

    fn summary(s: &[f64], cores: usize) -> String {
        let (lo, mid, hi) = (percentile(s, 0.0), percentile(s, 0.5), percentile(s, 1.0));
        format!(
            "median={mid:.1}% of one core ({:.1}% of {cores} cores) min={lo:.1}% max={hi:.1}%",
            mid / cores as f64
        )
    }

    // Measurement, not a gate: `cargo test --release --lib --features gstreamer cpu_budget -- --ignored --nocapture`.
    #[test]
    #[ignore = "measurement, not a gate"]
    fn cpu_budget_with_four_1080p_sources() {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        // Same four NV12 sources into fakesinks: the generation/queue cost without any compositing.
        let src = |p: &str| {
            format!("videotestsrc is-live=true pattern={p} ! video/x-raw,width=1920,height=1080,framerate=30/1,format=NV12 ! queue max-size-buffers=3 leaky=downstream ! fakesink sync=true")
        };
        let desc = ["smpte", "ball", "pinwheel", "checkers-8"]
            .map(src)
            .join(" ");
        gst::init().expect("gst init");
        let baseline = gst::parse::launch(&desc).expect("baseline pipeline");
        baseline.set_state(gst::State::Playing).expect("play");
        sleep(2000);
        let base = process_cpu_samples(6);
        let _ = baseline.set_state(gst::State::Null);
        println!("[sources only, no compositor] {}", summary(&base, cores));
        for (label, filter) in [("default", None), ("cpu", Some(cpu_only as Filter))] {
            let (mut b, _bus) = backend(filter);
            let info = b.compositor_start(&layout(4)).expect("start");
            let stamps = stamp_pad(&out_pad(b.compositor().expect("running")));
            sleep(2000);
            let (frames_before, t) = (stamps.lock().expect("stamps").len(), Instant::now());
            let cpu = process_cpu_samples(6);
            let frames = stamps.lock().expect("stamps").len() - frames_before;
            println!(
                "[{label}] element={} {} out_fps={:.1}",
                info.element,
                summary(&cpu, cores),
                frames as f64 / t.elapsed().as_secs_f64()
            );
            b.shutdown();
        }
    }
}
