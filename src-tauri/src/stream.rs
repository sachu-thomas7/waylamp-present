//! F4: live-streaming egress. Encode once, fan out to any number of RTMP(S) destinations
//! (Facebook, YouTube, custom). This file is the pure core: config, validation, encoder choice,
//! pipeline descriptions, reconnect back-off. The engine behind `StreamBackend` lives elsewhere.
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const MAX_DESTINATIONS: usize = 4;
const MAX_KEY_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Facebook,
    Youtube,
    Custom,
}

impl Platform {
    /// Ingest URL pre-filled in the UI; the operator may override it (e.g. YouTube's backup ingest).
    pub fn default_ingest_url(self) -> &'static str {
        match self {
            Platform::Facebook => "rtmps://live-api-s.facebook.com:443/rtmp/",
            Platform::Youtube => "rtmp://a.rtmp.youtube.com/live2",
            Platform::Custom => "",
        }
    }
}

/// One output of the stream. The stream key is a secret and lives in the keychain under `id`, never here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Destination {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub ingest_url: String,
    pub enabled: bool,
}

impl Default for Destination {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            platform: Platform::Custom,
            ingest_url: String::new(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub video_kbps: u32,
    pub audio_kbps: u32,
    /// Seconds between keyframes; both platforms want 2 s.
    pub keyframe_secs: u32,
    /// Force a specific encoder element (testing / odd drivers); empty means automatic.
    pub encoder: String,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            video_kbps: 6000,
            audio_kbps: 128,
            keyframe_secs: 2,
            encoder: String::new(),
        }
    }
}

impl StreamSettings {
    pub fn validate(&self) -> Result<(), AppError> {
        let bad = |m: &str| Err(AppError::InvalidInput(m.into()));
        if !(320..=3840).contains(&self.width) || !(240..=2160).contains(&self.height) {
            return bad("resolution must be between 320x240 and 3840x2160");
        }
        if !(1..=60).contains(&self.fps) {
            return bad("fps must be between 1 and 60");
        }
        if !(300..=51_000).contains(&self.video_kbps) {
            return bad("video bitrate must be between 300 and 51000 kbps");
        }
        if !(32..=320).contains(&self.audio_kbps) {
            return bad("audio bitrate must be between 32 and 320 kbps");
        }
        if !(1..=10).contains(&self.keyframe_secs) {
            return bad("keyframe interval must be between 1 and 10 seconds");
        }
        Ok(())
    }
}

pub fn validate_destination_id(id: &str) -> Result<(), AppError> {
    let ok = !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "destination id must be 1-32 characters of a-z, 0-9, '-' or '_'".into(),
        ))
    }
}

pub fn validate_stream_key(key: &str) -> Result<(), AppError> {
    let ok =
        !key.is_empty() && key.len() <= MAX_KEY_LEN && key.bytes().all(|b| b.is_ascii_graphic());
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "stream key must be 1-512 printable characters with no spaces".into(),
        ))
    }
}

pub fn validate_destination(d: &Destination) -> Result<(), AppError> {
    validate_destination_id(&d.id)?;
    if d.name.trim().is_empty() || d.name.len() > 64 {
        return Err(AppError::InvalidInput(
            "destination needs a name (max 64 characters)".into(),
        ));
    }
    let url = d.ingest_url.trim();
    let scheme_ok = url.starts_with("rtmp://") || url.starts_with("rtmps://");
    let clean = url
        .bytes()
        .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\');
    if !scheme_ok || url.len() > 512 || !clean {
        return Err(AppError::InvalidInput(
            "ingest URL must start with rtmp:// or rtmps:// and contain no spaces or quotes".into(),
        ));
    }
    Ok(())
}

pub fn validate_destinations(list: &[Destination]) -> Result<(), AppError> {
    if list.len() > MAX_DESTINATIONS {
        return Err(AppError::InvalidInput(format!(
            "at most {MAX_DESTINATIONS} destinations"
        )));
    }
    for (i, d) in list.iter().enumerate() {
        validate_destination(d)?;
        if list[..i].iter().any(|o| o.id == d.id) {
            return Err(AppError::InvalidInput(format!(
                "duplicate destination id: {}",
                d.id
            )));
        }
    }
    Ok(())
}

/// The URL handed to the RTMP sink: ingest base plus the stream key. Contains the secret; never log it.
pub fn full_url(ingest_url: &str, key: &str) -> String {
    format!("{}/{}", ingest_url.trim().trim_end_matches('/'), key)
}

// ── Encoder choice ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncoderChoice {
    pub element: &'static str,
    pub hardware: bool,
}

/// Preference order. x264enc is last and only when allowed: it is GPL and must not ship in a closed bundle.
const ENCODER_PREFERENCE: &[(&str, bool)] = &[
    ("nvh264enc", true),
    ("qsvh264enc", true),
    ("amfh264enc", true),
    ("mfh264enc", true),
    ("vtenc_h264", true),
    ("vah264enc", true),
    ("openh264enc", false),
    ("x264enc", false),
];

pub fn select_encoder(
    available: &dyn Fn(&str) -> bool,
    forced: &str,
    allow_gpl: bool,
) -> Result<EncoderChoice, AppError> {
    let mut candidates = ENCODER_PREFERENCE
        .iter()
        .filter(|(e, _)| allow_gpl || *e != "x264enc")
        .filter(|(e, _)| forced.is_empty() || *e == forced)
        .filter(|(e, _)| available(e))
        .map(|(e, hw)| EncoderChoice {
            element: e,
            hardware: *hw,
        });
    candidates.next().ok_or_else(|| {
        if forced.is_empty() {
            AppError::Internal(
                "no H.264 encoder found (need a hardware encoder, openh264enc or x264enc)".into(),
            )
        } else {
            AppError::InvalidInput(format!("encoder '{forced}' is not available"))
        }
    })
}

/// Properties per encoder for CBR live output with a keyframe every `keyframe_secs`.
pub fn encoder_description(c: &EncoderChoice, s: &StreamSettings) -> String {
    let gop = s.fps * s.keyframe_secs;
    let kbps = s.video_kbps;
    match c.element {
        "nvh264enc" => format!(
            "nvh264enc preset=low-latency-hq rc-mode=cbr bitrate={kbps} gop-size={gop} zerolatency=true bframes=0"
        ),
        "qsvh264enc" => format!("qsvh264enc bitrate={kbps} gop-size={gop} rate-control=cbr"),
        "amfh264enc" => format!("amfh264enc bitrate={kbps} gop-size={gop} rate-control=cbr"),
        "mfh264enc" => format!(
            "mfh264enc bitrate={kbps} gop-size={gop} rc-mode=cbr low-latency=true bframes=0"
        ),
        "vtenc_h264" => format!(
            "vtenc_h264 bitrate={kbps} max-keyframe-interval={gop} realtime=true allow-frame-reordering=false"
        ),
        "vah264enc" => format!("vah264enc bitrate={kbps} key-int-max={gop} rate-control=cbr"),
        "openh264enc" => format!(
            "openh264enc bitrate={} gop-size={gop} rate-control=bitrate complexity=low",
            kbps * 1000
        ),
        _ => format!(
            "x264enc bitrate={kbps} key-int-max={gop} tune=zerolatency speed-preset=veryfast bframes=0 byte-stream=true"
        ),
    }
}

// ── Pipeline descriptions (pure strings; the Gst backend parses them) ───────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum VideoSource {
    /// A moving test pattern, for tests and "is my key valid" checks.
    TestPattern,
    /// The compositor's program output, shared in-process through `intervideosink channel=<name>`.
    Program(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum AudioSource {
    /// Silent track: platforms reject or flag video-only streams, so there is always an audio track.
    Silence,
    /// Any OS audio capture device by id (WASAPI/CoreAudio/PulseAudio): wireless-mic receivers, mixers, interfaces.
    Device(String),
}

fn safe_prop(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || "-_.{}:/\\ ".contains(*c))
        .collect()
}

pub fn video_head(src: &VideoSource, s: &StreamSettings) -> String {
    let caps = format!(
        "video/x-raw,width={},height={},framerate={}/1",
        s.width, s.height, s.fps
    );
    let source = match src {
        VideoSource::TestPattern => "videotestsrc is-live=true pattern=ball".to_string(),
        VideoSource::Program(ch) => {
            format!("intervideosrc channel={}", safe_prop(ch).replace(' ', ""))
        }
    };
    format!("{source} ! videoconvert ! videoscale ! videorate ! {caps} ! videoconvert")
}

pub fn audio_head(src: &AudioSource) -> String {
    let source = match src {
        AudioSource::Silence => "audiotestsrc is-live=true wave=silence".to_string(),
        AudioSource::Device(id) => format!(
            "wasapi2src device=\"{}\" low-latency=true",
            safe_prop(id).replace('\\', "\\\\")
        ),
    };
    format!("{source} ! audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2")
}

// ── Reconnect back-off (1 s, 2 s, 4 s ... capped) ───────────────────────────

#[derive(Debug, Clone)]
pub struct Backoff {
    attempt: u32,
    base: Duration,
    cap: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            attempt: 0,
            base: Duration::from_secs(1),
            cap: Duration::from_secs(30),
        }
    }
}

impl Backoff {
    pub fn next_delay(&mut self) -> Duration {
        let d = self
            .base
            .saturating_mul(1u32 << self.attempt.min(16))
            .min(self.cap);
        self.attempt = self.attempt.saturating_add(1);
        d
    }
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}

// ── State reported to the UI ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "detail")]
pub enum DestinationState {
    Idle,
    Connecting,
    Live,
    Reconnecting { attempt: u32, retry_in_secs: u64 },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum StreamState {
    Stopped,
    Starting,
    Live,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DestinationStatus {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub state: DestinationState,
    pub bitrate_kbps: u32,
    pub dropped_frames: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StreamStatus {
    pub state: StreamState,
    pub encoder: Option<String>,
    pub hardware: bool,
    pub started_unix_ms: Option<u64>,
    pub destinations: Vec<DestinationStatus>,
}

impl StreamStatus {
    pub fn stopped() -> Self {
        Self {
            state: StreamState::Stopped,
            encoder: None,
            hardware: false,
            started_unix_ms: None,
            destinations: Vec::new(),
        }
    }
}

/// Removes secrets from text before it can reach a log, event or error message.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    secrets
        .iter()
        .filter(|s| s.len() >= 4)
        .fold(text.to_string(), |acc, s| acc.replace(s, "***"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dest(id: &str, url: &str) -> Destination {
        Destination {
            id: id.into(),
            name: format!("{id} name"),
            platform: Platform::Custom,
            ingest_url: url.into(),
            enabled: true,
        }
    }

    #[test]
    fn destination_validation_accepts_rtmp_and_rtmps_only() {
        assert!(validate_destination(&dest("yt", "rtmp://a.rtmp.youtube.com/live2")).is_ok());
        assert!(
            validate_destination(&dest("fb", "rtmps://live-api-s.facebook.com:443/rtmp/")).is_ok()
        );
        for bad in [
            "http://x/y",
            "rtmp://has space/x",
            "rtmp://x/\"y",
            "",
            "srt://x:1",
        ] {
            assert!(validate_destination(&dest("a", bad)).is_err(), "{bad}");
        }
        assert!(validate_destination(&dest("Bad Id", "rtmp://x/y")).is_err());
        let unnamed = Destination {
            name: " ".into(),
            ..dest("a", "rtmp://x/y")
        };
        assert!(validate_destination(&unnamed).is_err());
    }

    #[test]
    fn destination_list_rules() {
        let a = dest("a", "rtmp://x/y");
        assert!(validate_destinations(&[a.clone(), dest("b", "rtmp://x/y")]).is_ok());
        assert!(
            validate_destinations(&[a.clone(), a.clone()]).is_err(),
            "duplicate id"
        );
        let many: Vec<_> = (0..=MAX_DESTINATIONS)
            .map(|i| dest(&format!("d{i}"), "rtmp://x/y"))
            .collect();
        assert!(validate_destinations(&many).is_err());
    }

    #[test]
    fn stream_key_must_be_printable_without_spaces() {
        assert!(validate_stream_key("abcd-1234_efgh").is_ok());
        for bad in ["", "has space", "tab\t", "new\nline", &"x".repeat(513)] {
            assert!(validate_stream_key(bad).is_err());
        }
    }

    #[test]
    fn full_url_joins_without_double_slashes() {
        assert_eq!(
            full_url("rtmps://h:443/rtmp/", "KEY"),
            "rtmps://h:443/rtmp/KEY"
        );
        assert_eq!(full_url(" rtmp://h/live2 ", "KEY"), "rtmp://h/live2/KEY");
    }

    #[test]
    fn platform_defaults_are_the_documented_ingests() {
        assert!(Platform::Facebook
            .default_ingest_url()
            .starts_with("rtmps://live-api-s.facebook.com"));
        assert!(Platform::Youtube
            .default_ingest_url()
            .starts_with("rtmp://a.rtmp.youtube.com"));
        assert_eq!(Platform::Custom.default_ingest_url(), "");
    }

    #[test]
    fn settings_validation_bounds() {
        assert!(StreamSettings::default().validate().is_ok());
        let d = StreamSettings::default;
        for bad in [
            StreamSettings { fps: 0, ..d() },
            StreamSettings { fps: 120, ..d() },
            StreamSettings {
                video_kbps: 100,
                ..d()
            },
            StreamSettings { width: 100, ..d() },
            StreamSettings {
                audio_kbps: 8,
                ..d()
            },
            StreamSettings {
                keyframe_secs: 0,
                ..d()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn encoder_prefers_hardware_then_software_and_gates_gpl() {
        let all = |_: &str| true;
        assert_eq!(select_encoder(&all, "", true).unwrap().element, "nvh264enc");
        let only_sw = |e: &str| e == "x264enc" || e == "openh264enc";
        assert_eq!(
            select_encoder(&only_sw, "", true).unwrap().element,
            "openh264enc"
        );
        let only_x264 = |e: &str| e == "x264enc";
        assert!(
            select_encoder(&only_x264, "", false).is_err(),
            "GPL x264 only when allowed"
        );
        assert_eq!(
            select_encoder(&only_x264, "", true).unwrap().element,
            "x264enc"
        );
        let mf = |e: &str| e == "mfh264enc" || e == "x264enc";
        let c = select_encoder(&mf, "", true).unwrap();
        assert!(c.hardware && c.element == "mfh264enc");
    }

    #[test]
    fn forced_encoder_must_be_known_and_available() {
        let all = |_: &str| true;
        assert_eq!(
            select_encoder(&all, "mfh264enc", true).unwrap().element,
            "mfh264enc"
        );
        assert!(
            select_encoder(&all, "evil ! filesrc", true).is_err(),
            "never splices arbitrary text into a pipeline"
        );
        let none = |_: &str| false;
        assert!(select_encoder(&none, "nvh264enc", true).is_err());
        assert!(select_encoder(&none, "", true).is_err());
    }

    #[test]
    fn encoder_descriptions_carry_bitrate_and_a_two_second_gop() {
        let s = StreamSettings::default();
        let d = |e: &'static str| {
            encoder_description(
                &EncoderChoice {
                    element: e,
                    hardware: true,
                },
                &s,
            )
        };
        assert!(d("nvh264enc").contains("bitrate=6000") && d("nvh264enc").contains("gop-size=60"));
        assert!(d("openh264enc").contains("bitrate=6000000"));
        assert!(d("x264enc").contains("key-int-max=60") && d("x264enc").contains("zerolatency"));
        assert!(d("mfh264enc").contains("bframes=0"));
    }

    #[test]
    fn heads_describe_the_chosen_sources_and_cannot_be_escaped() {
        let s = StreamSettings::default();
        let v = video_head(&VideoSource::Program("program".into()), &s);
        assert!(
            v.starts_with("intervideosrc channel=program")
                && v.contains("width=1920,height=1080,framerate=30/1")
        );
        let evil = video_head(
            &VideoSource::Program("x ! filesrc location=/etc/passwd".into()),
            &s,
        );
        assert!(
            evil.starts_with("intervideosrc channel=xfilesrc") && !evil.contains("! filesrc"),
            "{evil}"
        );
        assert!(video_head(&VideoSource::TestPattern, &s).starts_with("videotestsrc"));
        assert!(audio_head(&AudioSource::Silence).contains("wave=silence"));
        let a = audio_head(&AudioSource::Device("{0.0.1}.\"x\" ! filesink".into()));
        assert!(
            a.contains("wasapi2src device=\"") && !a.contains("\"x\""),
            "{a}"
        );
    }

    #[test]
    fn backoff_doubles_then_caps_and_resets() {
        let mut b = Backoff::default();
        let secs: Vec<u64> = (0..8).map(|_| b.next_delay().as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(b.attempts(), 8);
        b.reset();
        assert_eq!(b.next_delay().as_secs(), 1);
    }

    #[test]
    fn redact_removes_every_secret_occurrence() {
        let msg = "failed rtmps://h/live/SECRETKEY: SECRETKEY rejected";
        let out = redact(msg, &["SECRETKEY"]);
        assert!(!out.contains("SECRETKEY") && out.contains("***"));
        assert_eq!(
            redact("abc", &["ab"]),
            "abc",
            "too-short secrets are ignored, not mangled"
        );
    }

    #[test]
    fn destination_config_round_trips_through_toml_without_a_key_field() {
        #[derive(Serialize, Deserialize)]
        struct Wrap {
            destinations: Vec<Destination>,
        }
        let list = vec![dest("fb", "rtmps://live-api-s.facebook.com:443/rtmp/")];
        let text = toml::to_string(&Wrap {
            destinations: list.clone(),
        })
        .unwrap();
        assert!(!text.to_lowercase().contains("key"));
        let back: Wrap = toml::from_str(&text).unwrap();
        assert_eq!(back.destinations, list);
    }
}
