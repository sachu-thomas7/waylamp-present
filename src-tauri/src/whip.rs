use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::{Duration, Instant};

use crate::bus::EventBus;
use crate::config_loader::ConfigLoader;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::ndi::{NdiSource, StreamFormat};
use crate::preview::{PreviewHub, SimulatedFeeds};

pub const MAX_SESSIONS: usize = 4;
pub const GRACE_PERIOD: Duration = Duration::from_secs(10);
pub const UI_SLOTS: [&str; MAX_SESSIONS] = ["camera1", "camera2", "camera3", "camera4"];
pub const KEYCHAIN_SERVICE: &str = "streamdeck";
pub const KEYCHAIN_USER: &str = "whip-bearer-token";
const MAX_TOKEN_LEN: usize = 256;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Failure of a WHIP request; the HTTP layer maps each kind to a status code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhipError {
    BadRequest(String),
    Unauthorized,
    NotFound(String),
    Conflict(String),
    Unsupported(String),
    Busy(String),
    Unavailable(String),
    Internal(String),
}

impl std::fmt::Display for WhipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("missing or invalid bearer token"),
            Self::BadRequest(m)
            | Self::NotFound(m)
            | Self::Conflict(m)
            | Self::Unsupported(m)
            | Self::Busy(m)
            | Self::Unavailable(m)
            | Self::Internal(m) => f.write_str(m),
        }
    }
}

impl From<AppError> for WhipError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::InvalidInput(m) => Self::BadRequest(m),
            AppError::NotFound(m) => Self::NotFound(m),
            other => Self::Internal(other.to_string()),
        }
    }
}

/// `camera<N>` with N in 1..=99; the slot name is also the source name in the grid.
pub fn parse_slot(raw: &str) -> Option<String> {
    let n = raw.strip_prefix("camera")?;
    let ok = !n.is_empty()
        && n.len() <= 2
        && n.bytes().all(|b| b.is_ascii_digit())
        && !n.starts_with('0');
    ok.then(|| raw.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum WhipCodec {
    H264Baseline,
    Vp8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecChoice {
    pub codec: WhipCodec,
    pub payload_type: u8,
}

/// RFC 6184: baseline is profile_idc 0x42, or a Main/Extended stream with constraint_set0 set.
fn is_baseline(profile_level_id: &str) -> bool {
    let byte = |i: usize| {
        profile_level_id
            .get(i * 2..i * 2 + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
    };
    match (profile_level_id.len(), byte(0), byte(1)) {
        (6, Some(idc), Some(iop)) => idc == 0x42 || (matches!(idc, 0x4d | 0x58) && iop & 0x80 != 0),
        _ => false,
    }
}

fn fmtp_param<'a>(params: &'a str, key: &str) -> Option<&'a str> {
    params.split(';').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim())
    })
}

fn offered_with(offered: &[u8], names: &HashMap<u8, String>, wanted: &str) -> Vec<u8> {
    offered
        .iter()
        .copied()
        .filter(|pt| names.get(pt).is_some_and(|n| n == wanted))
        .collect()
}

/// Picks H.264 Baseline (packetization-mode 1 first), else VP8, from the offer's first video section.
pub fn select_codec(offer_sdp: &str) -> Result<CodecChoice, WhipError> {
    let mut in_video = false;
    let mut offered: Vec<u8> = Vec::new();
    let mut names: HashMap<u8, String> = HashMap::new();
    let mut fmtp: HashMap<u8, String> = HashMap::new();
    let mut seen_video = false;
    for line in offer_sdp.lines().map(str::trim) {
        if let Some(m) = line.strip_prefix("m=") {
            in_video = !seen_video && m.starts_with("video ");
            if in_video {
                seen_video = true;
                offered = m
                    .split_whitespace()
                    .skip(3)
                    .filter_map(|p| p.parse().ok())
                    .collect();
            }
        } else if in_video {
            if let Some(r) = line.strip_prefix("a=rtpmap:") {
                if let Some((pt, enc)) = r.split_once(' ') {
                    if let Ok(pt) = pt.parse::<u8>() {
                        let name = enc.split('/').next().unwrap_or("").to_ascii_uppercase();
                        names.insert(pt, name);
                    }
                }
            } else if let Some(r) = line.strip_prefix("a=fmtp:") {
                if let Some((pt, params)) = r.split_once(' ') {
                    if let Ok(pt) = pt.parse::<u8>() {
                        fmtp.insert(pt, params.to_string());
                    }
                }
            }
        }
    }
    if !seen_video {
        return Err(WhipError::BadRequest("offer has no video section".into()));
    }
    let baseline: Vec<u8> = offered_with(&offered, &names, "H264")
        .into_iter()
        .filter(|pt| {
            let params = fmtp.get(pt).map(String::as_str).unwrap_or("");
            // A missing profile-level-id defaults to baseline (RFC 6184 §8.1).
            fmtp_param(params, "profile-level-id").map_or(true, is_baseline)
        })
        .collect();
    let mode1 = baseline.iter().copied().find(|pt| {
        let params = fmtp.get(pt).map(String::as_str).unwrap_or("");
        fmtp_param(params, "packetization-mode") == Some("1")
    });
    if let Some(pt) = mode1.or_else(|| baseline.first().copied()) {
        return Ok(CodecChoice {
            codec: WhipCodec::H264Baseline,
            payload_type: pt,
        });
    }
    if let Some(pt) = offered_with(&offered, &names, "VP8").first().copied() {
        return Ok(CodecChoice {
            codec: WhipCodec::Vp8,
            payload_type: pt,
        });
    }
    Err(WhipError::Unsupported(
        "offer has neither H.264 Baseline nor VP8 video".into(),
    ))
}

fn offer_mid(offer_sdp: &str) -> String {
    let mut in_video = false;
    for line in offer_sdp.lines().map(str::trim) {
        if line.starts_with("m=") {
            in_video = line.starts_with("m=video ");
        } else if in_video {
            if let Some(mid) = line.strip_prefix("a=mid:") {
                return mid.to_string();
            }
        }
    }
    "0".into()
}

/// A canned receive-only answer for the chosen codec; only the simulated backend uses it.
pub fn canned_answer(offer_sdp: &str, choice: &CodecChoice) -> String {
    let (pt, mid) = (choice.payload_type, offer_mid(offer_sdp));
    let codec_lines = match choice.codec {
        WhipCodec::H264Baseline => format!(
            "a=rtpmap:{pt} H264/90000\r\na=fmtp:{pt} level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f\r\n"
        ),
        WhipCodec::Vp8 => format!("a=rtpmap:{pt} VP8/90000\r\n"),
    };
    format!(
        "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE {mid}\r\n\
         m=video 9 UDP/TLS/RTP/SAVPF {pt}\r\nc=IN IP4 0.0.0.0\r\na=mid:{mid}\r\na=recvonly\r\na=rtcp-mux\r\n{codec_lines}"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum WhipLinkState {
    Connected,
    Disconnected,
    Lost,
}

/// Pure session link model: a dropped ICE connection gets `grace` to come back before the session is lost.
#[derive(Debug, Clone)]
pub struct LinkMachine {
    state: WhipLinkState,
    disconnected_at: Option<Instant>,
    grace: Duration,
}

impl LinkMachine {
    pub fn new(grace: Duration) -> Self {
        Self {
            state: WhipLinkState::Connected,
            disconnected_at: None,
            grace,
        }
    }

    pub fn state(&self) -> WhipLinkState {
        self.state
    }

    /// Returns the new state when the ICE signal changed it; `Lost` is terminal.
    pub fn on_ice(&mut self, connected: bool, now: Instant) -> Option<WhipLinkState> {
        match (self.state, connected) {
            (WhipLinkState::Connected, false) => {
                self.state = WhipLinkState::Disconnected;
                self.disconnected_at = Some(now);
                Some(self.state)
            }
            (WhipLinkState::Disconnected, true) => {
                self.state = WhipLinkState::Connected;
                self.disconnected_at = None;
                Some(self.state)
            }
            _ => None,
        }
    }

    pub fn tick(&mut self, now: Instant) -> Option<WhipLinkState> {
        let since = self.disconnected_at?;
        if self.state == WhipLinkState::Disconnected
            && now.saturating_duration_since(since) >= self.grace
        {
            self.state = WhipLinkState::Lost;
            self.disconnected_at = None;
            return Some(self.state);
        }
        None
    }
}

/// Where this machine's LAN addresses come from; injected so tests need no network.
pub trait LocalIpProvider: Send + Sync + 'static {
    fn candidates(&self) -> Vec<IpAddr>;
}

/// Asks the OS which interface would route to the internet; a UDP connect sends no packets.
pub struct SystemLocalIp;

impl LocalIpProvider for SystemLocalIp {
    fn candidates(&self) -> Vec<IpAddr> {
        let probe = || -> std::io::Result<IpAddr> {
            let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
            socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80))?;
            Ok(socket.local_addr()?.ip())
        };
        probe().into_iter().collect()
    }
}

/// A fixed answer, for tests and harnesses.
pub struct FixedLocalIp(pub Vec<IpAddr>);

impl LocalIpProvider for FixedLocalIp {
    fn candidates(&self) -> Vec<IpAddr> {
        self.0.clone()
    }
}

/// Private IPv4 first, then any routable IPv4; never loopback, link-local or unspecified.
pub fn pick_lan_ip(candidates: &[IpAddr]) -> Option<IpAddr> {
    let v4: Vec<Ipv4Addr> = candidates
        .iter()
        .filter_map(|ip| match ip {
            IpAddr::V4(v) => Some(*v),
            IpAddr::V6(_) => None,
        })
        .filter(|v| !v.is_loopback() && !v.is_link_local() && !v.is_unspecified())
        .collect();
    v4.iter()
        .find(|v| v.is_private())
        .or_else(|| v4.first())
        .map(|v| IpAddr::V4(*v))
}

pub fn whip_url(ip: IpAddr, port: u16, slot: &str) -> String {
    match ip {
        IpAddr::V4(v) => format!("http://{v}:{port}/whip/{slot}"),
        IpAddr::V6(v) => format!("http://[{v}]:{port}/whip/{slot}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct QrMatrix {
    pub size: u32,
    pub modules: Vec<bool>,
}

pub fn qr_matrix(text: &str) -> Result<QrMatrix, AppError> {
    let code = qrcode::QrCode::new(text.as_bytes())
        .map_err(|e| AppError::Internal(format!("QR encode failed: {e}")))?;
    Ok(QrMatrix {
        size: code.width() as u32,
        modules: code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect(),
    })
}

/// A bearer token must survive a header value: printable ASCII, no spaces, bounded length.
pub fn validate_token(token: &str) -> Result<(), AppError> {
    let ok = !token.is_empty()
        && token.len() <= MAX_TOKEN_LEN
        && token.bytes().all(|b| b.is_ascii_graphic());
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "token must be 1-{MAX_TOKEN_LEN} printable characters with no spaces"
        )))
    }
}

/// Constant-time over the shorter length plus a length check, so timing does not leak the token.
fn tokens_match(expected: &str, given: &str) -> bool {
    let (a, b) = (expected.as_bytes(), given.as_bytes());
    let diff = a
        .iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y));
    a.len() == b.len() && diff == 0
}

/// Secret storage for the ingest token — the OS keychain in production, never TOML.
pub trait TokenStore: Send + Sync + 'static {
    fn get(&self) -> Result<Option<String>, AppError>;
    fn set(&self, token: Option<&str>) -> Result<(), AppError>;
}

pub struct KeyringTokenStore;

impl KeyringTokenStore {
    fn entry() -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)
            .map_err(|e| AppError::Keychain(e.to_string()))
    }
}

impl TokenStore for KeyringTokenStore {
    fn get(&self) -> Result<Option<String>, AppError> {
        match Self::entry()?.get_password() {
            Ok(t) => Ok(Some(t)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Keychain(e.to_string())),
        }
    }

    fn set(&self, token: Option<&str>) -> Result<(), AppError> {
        let entry = Self::entry()?;
        match token {
            Some(t) => entry
                .set_password(t)
                .map_err(|e| AppError::Keychain(e.to_string())),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(AppError::Keychain(e.to_string())),
            },
        }
    }
}

/// In-memory store for tests and the mock harness.
#[derive(Default)]
pub struct MemoryTokenStore {
    token: Mutex<Option<String>>,
    fail: Mutex<Option<String>>,
}

impl MemoryTokenStore {
    pub fn failing(&self, reason: Option<&str>) {
        *lock(&self.fail) = reason.map(String::from);
    }
}

impl TokenStore for MemoryTokenStore {
    fn get(&self) -> Result<Option<String>, AppError> {
        if let Some(r) = lock(&self.fail).clone() {
            return Err(AppError::Keychain(r));
        }
        Ok(lock(&self.token).clone())
    }

    fn set(&self, token: Option<&str>) -> Result<(), AppError> {
        if let Some(r) = lock(&self.fail).clone() {
            return Err(AppError::Keychain(r));
        }
        *lock(&self.token) = token.map(String::from);
        Ok(())
    }
}

/// Called by a backend with `(slot, ice_connected)` whenever a session's ICE connectivity changes.
pub type IceListener = Arc<dyn Fn(&str, bool) + Send + Sync>;

/// The WebRTC engine behind the endpoint; methods may block, so callers run them off the async threads.
pub trait WhipBackend: Send + Sync + 'static {
    /// Accepts the offer for `slot`, starts feeding its preview, and returns the SDP answer.
    fn negotiate(&self, slot: &str, offer: &str, choice: &CodecChoice) -> Result<String, AppError>;
    /// Idempotent.
    fn teardown(&self, slot: &str);
    /// Handles an ICE-restart PATCH body for a live session.
    fn ice_restart(&self, _slot: &str, _sdp_fragment: &str) -> Result<(), AppError> {
        Ok(())
    }
    /// Why this build can never negotiate, if so.
    fn unavailable_reason(&self) -> Option<String> {
        None
    }
    /// Registers where the backend reports ICE connectivity changes; called once at startup.
    fn set_ice_listener(&self, _listener: IceListener) {}
}

/// Used when no WebRTC engine is compiled in; the endpoint answers 503 with this reason.
pub struct UnavailableWhipBackend(pub String);

impl WhipBackend for UnavailableWhipBackend {
    fn negotiate(&self, _: &str, _: &str, _: &CodecChoice) -> Result<String, AppError> {
        Err(AppError::Internal(self.0.clone()))
    }
    fn teardown(&self, _: &str) {}
    fn unavailable_reason(&self) -> Option<String> {
        Some(self.0.clone())
    }
}

/// Answers every offer with a canned SDP and (optionally) feeds synthetic frames into the preview.
#[derive(Clone, Default)]
pub struct SimulatedWhipBackend {
    log: Arc<Mutex<Vec<String>>>,
    fail_next: Arc<Mutex<Option<String>>>,
    feeds: Option<Arc<Mutex<SimulatedFeeds>>>,
    listener: Arc<Mutex<Option<IceListener>>>,
}

impl SimulatedWhipBackend {
    pub fn with_preview(hub: PreviewHub, fps: u32) -> Self {
        Self {
            feeds: Some(Arc::new(Mutex::new(SimulatedFeeds::new(hub, fps)))),
            ..Self::default()
        }
    }

    pub fn calls(&self) -> Vec<String> {
        lock(&self.log).clone()
    }

    pub fn fail_next_negotiate(&self, reason: &str) {
        *lock(&self.fail_next) = Some(reason.into());
    }

    /// Plays the part of the WebRTC stack reporting an ICE change for `slot`.
    pub fn simulate_ice(&self, slot: &str, connected: bool) {
        let listener = lock(&self.listener).clone();
        if let Some(l) = listener {
            l(slot, connected);
        }
    }

    fn record(&self, entry: String) {
        lock(&self.log).push(entry);
    }
}

impl WhipBackend for SimulatedWhipBackend {
    fn negotiate(&self, slot: &str, offer: &str, choice: &CodecChoice) -> Result<String, AppError> {
        self.record(format!("negotiate:{slot}:{:?}", choice.codec));
        if let Some(reason) = lock(&self.fail_next).take() {
            return Err(AppError::Internal(reason));
        }
        if let Some(feeds) = &self.feeds {
            lock(feeds).start(slot);
        }
        Ok(canned_answer(offer, choice))
    }

    fn teardown(&self, slot: &str) {
        self.record(format!("teardown:{slot}"));
        if let Some(feeds) = &self.feeds {
            lock(feeds).stop(slot);
        }
    }

    fn ice_restart(&self, slot: &str, _: &str) -> Result<(), AppError> {
        self.record(format!("ice_restart:{slot}"));
        Ok(())
    }

    fn set_ice_listener(&self, listener: IceListener) {
        *lock(&self.listener) = Some(listener);
    }
}

struct Session {
    id: String,
    codec: WhipCodec,
    link: LinkMachine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    pub session_id: String,
    pub answer: String,
    pub codec: WhipCodec,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SessionInfo {
    pub slot: String,
    pub state: WhipLinkState,
    pub codec: WhipCodec,
}

/// Session registry behind the HTTP endpoint; every state change is published on the bus.
pub struct WhipService {
    sessions: Mutex<BTreeMap<String, Session>>,
    backend: Arc<dyn WhipBackend>,
    bus: Arc<EventBus>,
    token: RwLock<Option<String>>,
    grace: Duration,
}

impl WhipService {
    pub fn new(backend: Arc<dyn WhipBackend>, bus: Arc<EventBus>) -> Arc<Self> {
        Self::with_grace(backend, bus, GRACE_PERIOD)
    }

    pub fn with_grace(
        backend: Arc<dyn WhipBackend>,
        bus: Arc<EventBus>,
        grace: Duration,
    ) -> Arc<Self> {
        Arc::new_cyclic(|weak: &std::sync::Weak<Self>| {
            let weak = weak.clone();
            backend.set_ice_listener(Arc::new(move |slot, connected| {
                if let Some(s) = weak.upgrade() {
                    s.on_backend_ice(slot, connected, Instant::now());
                }
            }));
            Self {
                sessions: Mutex::new(BTreeMap::new()),
                backend,
                bus,
                token: RwLock::new(None),
                grace,
            }
        })
    }

    /// Backend-reported ICE change for whatever session currently holds `slot`.
    fn on_backend_ice(&self, slot: &str, connected: bool, now: Instant) {
        let id = lock(&self.sessions).get(slot).map(|s| s.id.clone());
        if let Some(id) = id {
            let _ = self.ice_signal(slot, &id, connected, now);
        }
    }

    pub fn unavailable_reason(&self) -> Option<String> {
        self.backend.unavailable_reason()
    }

    pub fn set_token(&self, token: Option<String>) {
        *self.token.write().unwrap_or_else(PoisonError::into_inner) = token;
    }

    pub fn token_set(&self) -> bool {
        self.token
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// `header` is the raw `Authorization` value; with no token configured everything passes.
    pub fn authorize(&self, header: Option<&str>) -> Result<(), WhipError> {
        let guard = self.token.read().unwrap_or_else(PoisonError::into_inner);
        let Some(expected) = guard.as_deref() else {
            return Ok(());
        };
        let given = header
            .and_then(|h| h.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, t)| t.trim());
        match given {
            Some(t) if tokens_match(expected, t) => Ok(()),
            _ => Err(WhipError::Unauthorized),
        }
    }

    pub fn create_session(&self, slot: &str, offer: &str) -> Result<Created, WhipError> {
        if let Some(reason) = self.backend.unavailable_reason() {
            return Err(WhipError::Unavailable(reason));
        }
        let choice = select_codec(offer)?;
        let mut sessions = lock(&self.sessions);
        let replacing = match sessions.get(slot).map(|s| s.link.state()) {
            Some(WhipLinkState::Connected) => {
                return Err(WhipError::Conflict(format!(
                    "{slot} already has a live session"
                )))
            }
            Some(state) => Some(state),
            None => None,
        };
        let others = sessions.iter().filter(|(k, _)| k.as_str() != slot).count();
        if others >= MAX_SESSIONS {
            return Err(WhipError::Busy(format!(
                "all {MAX_SESSIONS} phone slots are in use"
            )));
        }
        if replacing.is_some_and(|s| s != WhipLinkState::Lost) {
            self.backend.teardown(slot);
        }
        let answer = self
            .backend
            .negotiate(slot, offer, &choice)
            .map_err(WhipError::from)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        sessions.insert(
            slot.to_string(),
            Session {
                id: id.clone(),
                codec: choice.codec,
                link: LinkMachine::new(self.grace),
            },
        );
        drop(sessions);
        if replacing.is_some() {
            self.bus.publish(AppEvent::SourceDiscovered {
                source: NdiSource::new(slot),
            });
        } else {
            self.bus.publish(AppEvent::SourceAdded {
                name: slot.into(),
                format: StreamFormat::Standard,
            });
        }
        self.bus.publish(AppEvent::WhipSessionChanged {
            slot: slot.into(),
            state: WhipLinkState::Connected,
        });
        Ok(Created {
            session_id: id,
            answer,
            codec: choice.codec,
        })
    }

    fn check_session(
        sessions: &BTreeMap<String, Session>,
        slot: &str,
        session_id: &str,
    ) -> Result<(), WhipError> {
        match sessions.get(slot) {
            Some(s) if s.id == session_id => Ok(()),
            _ => Err(WhipError::NotFound(format!("no such session on {slot}"))),
        }
    }

    pub fn delete_session(&self, slot: &str, session_id: &str) -> Result<(), WhipError> {
        let mut sessions = lock(&self.sessions);
        Self::check_session(&sessions, slot, session_id)?;
        sessions.remove(slot);
        drop(sessions);
        self.backend.teardown(slot);
        self.bus
            .publish(AppEvent::SourceRemoved { name: slot.into() });
        Ok(())
    }

    /// Feeds an ICE connectivity change into the session's link machine.
    pub fn ice_signal(
        &self,
        slot: &str,
        session_id: &str,
        connected: bool,
        now: Instant,
    ) -> Result<(), WhipError> {
        let mut sessions = lock(&self.sessions);
        Self::check_session(&sessions, slot, session_id)?;
        let changed = sessions
            .get_mut(slot)
            .and_then(|s| s.link.on_ice(connected, now));
        drop(sessions);
        if let Some(state) = changed {
            self.bus.publish(AppEvent::WhipSessionChanged {
                slot: slot.into(),
                state,
            });
        }
        Ok(())
    }

    /// Handles an ICE-restart PATCH: the backend renegotiates, then the link counts as back up.
    pub fn restart_ice(
        &self,
        slot: &str,
        session_id: &str,
        fragment: &str,
        now: Instant,
    ) -> Result<(), WhipError> {
        Self::check_session(&lock(&self.sessions), slot, session_id)?;
        let lost = lock(&self.sessions)
            .get(slot)
            .is_some_and(|s| s.link.state() == WhipLinkState::Lost);
        if lost {
            return Err(WhipError::NotFound(format!("{slot} session was lost")));
        }
        self.backend.ice_restart(slot, fragment)?;
        self.ice_signal(slot, session_id, true, now)
    }

    /// Expires sessions whose grace period ran out; the 1s task calls this.
    pub fn tick(&self, now: Instant) {
        let mut lost = Vec::new();
        for (slot, s) in lock(&self.sessions).iter_mut() {
            if s.link.tick(now).is_some() {
                lost.push(slot.clone());
            }
        }
        for slot in lost {
            self.backend.teardown(&slot);
            self.bus.publish(AppEvent::WhipSessionChanged {
                slot: slot.clone(),
                state: WhipLinkState::Lost,
            });
            self.bus.publish(AppEvent::SourceLost { name: slot });
        }
    }

    /// Ends every session (server stopped or app exiting).
    pub fn shutdown_all(&self) {
        let slots: Vec<String> = {
            let mut sessions = lock(&self.sessions);
            let keys = sessions.keys().cloned().collect();
            sessions.clear();
            keys
        };
        for slot in slots {
            self.backend.teardown(&slot);
            self.bus.publish(AppEvent::SourceRemoved { name: slot });
        }
    }

    pub fn sessions(&self) -> Vec<SessionInfo> {
        lock(&self.sessions)
            .iter()
            .map(|(slot, s)| SessionInfo {
                slot: slot.clone(),
                state: s.link.state(),
                codec: s.codec,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SlotInfo {
    pub slot: String,
    pub url: Option<String>,
    pub session: Option<SessionInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WhipInfo {
    pub enabled: bool,
    pub running: bool,
    pub port: u16,
    pub host: Option<String>,
    pub token_set: bool,
    pub unavailable: Option<String>,
    pub last_error: Option<String>,
    pub slots: Vec<SlotInfo>,
}

struct RunningServer {
    port: u16,
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tauri::async_runtime::JoinHandle<()>,
}

/// Owns the endpoint's lifecycle: config, token, the listening server and its session service.
pub struct WhipController {
    service: Arc<WhipService>,
    bus: Arc<EventBus>,
    config_path: PathBuf,
    ips: Arc<dyn LocalIpProvider>,
    tokens: Arc<dyn TokenStore>,
    server: tokio::sync::Mutex<Option<RunningServer>>,
    last_error: Mutex<Option<String>>,
}

impl WhipController {
    pub fn new(
        service: Arc<WhipService>,
        bus: Arc<EventBus>,
        config_path: PathBuf,
        ips: Arc<dyn LocalIpProvider>,
        tokens: Arc<dyn TokenStore>,
    ) -> Self {
        Self {
            service,
            bus,
            config_path,
            ips,
            tokens,
            server: tokio::sync::Mutex::new(None),
            last_error: Mutex::new(None),
        }
    }

    pub fn service(&self) -> &Arc<WhipService> {
        &self.service
    }

    /// Binds per `[whip]` and serves; the token is read first so a keychain failure keeps the endpoint closed.
    pub async fn start(&self) -> Result<SocketAddr, AppError> {
        let mut server = self.server.lock().await;
        if let Some(s) = server.as_ref() {
            return Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, s.port)));
        }
        let result = self.bind_and_serve().await;
        match &result {
            Ok((running, _)) => {
                *lock(&self.last_error) = None;
                self.bus.publish(AppEvent::WhipServerChanged {
                    running: true,
                    port: running.port,
                });
            }
            Err(e) => *lock(&self.last_error) = Some(e.to_string()),
        }
        let (running, addr) = result?;
        *server = Some(running);
        Ok(addr)
    }

    async fn bind_and_serve(&self) -> Result<(RunningServer, SocketAddr), AppError> {
        let config = ConfigLoader::load_or_default(&self.config_path)?.whip;
        self.service.set_token(self.tokens.get()?);
        let ip: IpAddr = config.bind_address.parse().map_err(|_| {
            AppError::Config(format!(
                "invalid [whip] bind_address: {}",
                config.bind_address
            ))
        })?;
        let listener = tokio::net::TcpListener::bind((ip, config.port))
            .await
            .map_err(|e| {
                AppError::Internal(format!("cannot listen on {ip}:{}: {e}", config.port))
            })?;
        let addr = listener
            .local_addr()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let (shutdown, rx) = tokio::sync::oneshot::channel();
        let task = crate::whip_server::spawn(listener, Arc::clone(&self.service), rx);
        Ok((
            RunningServer {
                port: addr.port(),
                shutdown,
                task,
            },
            addr,
        ))
    }

    /// Stops listening and ends every session.
    pub async fn stop(&self) {
        let running = self.server.lock().await.take();
        if let Some(r) = running {
            let _ = r.shutdown.send(());
            if tokio::time::timeout(Duration::from_secs(2), r.task)
                .await
                .is_err()
            {
                eprintln!("WHIP server did not stop within 2s");
            }
            self.service.shutdown_all();
            self.bus.publish(AppEvent::WhipServerChanged {
                running: false,
                port: r.port,
            });
        }
    }

    /// Persists `[whip] enabled` and starts or stops the server to match.
    pub async fn set_enabled(&self, enabled: bool) -> Result<(), AppError> {
        if enabled {
            self.start().await?;
        }
        let mut config = ConfigLoader::load_or_default(&self.config_path)?;
        config.whip.enabled = enabled;
        ConfigLoader::save(&self.config_path, &config)?;
        if !enabled {
            self.stop().await;
        }
        Ok(())
    }

    /// `None` clears the token (open endpoint); applies to the next request.
    pub fn set_token(&self, token: Option<String>) -> Result<(), AppError> {
        let token = token
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        if let Some(t) = &token {
            validate_token(t)?;
        }
        self.tokens.set(token.as_deref())?;
        self.service.set_token(token);
        Ok(())
    }

    pub async fn info(&self) -> Result<WhipInfo, AppError> {
        let config = ConfigLoader::load_or_default(&self.config_path)?.whip;
        let running_port = self.server.lock().await.as_ref().map(|s| s.port);
        let port = running_port.unwrap_or(config.port);
        let ip = pick_lan_ip(&self.ips.candidates());
        let sessions = self.service.sessions();
        Ok(WhipInfo {
            enabled: config.enabled,
            running: running_port.is_some(),
            port,
            host: ip.map(|i| i.to_string()),
            token_set: self.service.token_set() || self.tokens.get()?.is_some(),
            unavailable: self.service.unavailable_reason(),
            last_error: lock(&self.last_error).clone(),
            slots: UI_SLOTS
                .iter()
                .map(|slot| SlotInfo {
                    slot: (*slot).into(),
                    url: ip.map(|i| whip_url(i, port, slot)),
                    session: sessions.iter().find(|s| s.slot == *slot).cloned(),
                })
                .collect(),
        })
    }

    /// QR code for a slot's URL; errors when no LAN address is known.
    pub async fn qr(&self, slot: &str) -> Result<QrMatrix, AppError> {
        let slot = parse_slot(slot)
            .ok_or_else(|| AppError::InvalidInput(format!("invalid WHIP slot: {slot}")))?;
        let info = self.info().await?;
        let ip = pick_lan_ip(&self.ips.candidates())
            .ok_or_else(|| AppError::NotFound("no LAN address found".into()))?;
        qr_matrix(&whip_url(ip, info.port, &slot))
    }
}

/// Expires lost sessions once a second.
pub fn spawn_tick_task(service: Arc<WhipService>) -> tauri::async_runtime::JoinHandle<()> {
    // Called from .setup(), outside any Tokio runtime — tokio::spawn would panic here.
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let s = Arc::clone(&service);
            let _ = tokio::task::spawn_blocking(move || s.tick(Instant::now())).await;
        }
    })
}

/// The real engine under the `whip` feature; else unavailable, or simulated when `STREAMDECK_WHIP_SIMULATE=1`.
pub fn production_backend(preview: PreviewHub) -> Arc<dyn WhipBackend> {
    if std::env::var("STREAMDECK_WHIP_SIMULATE").as_deref() == Ok("1") {
        return Arc::new(SimulatedWhipBackend::with_preview(preview, 30));
    }
    real_backend(preview)
}

#[cfg(feature = "whip")]
fn real_backend(preview: PreviewHub) -> Arc<dyn WhipBackend> {
    match gst_whip::GstWhipBackend::new(preview) {
        Ok(b) => Arc::new(b),
        Err(e) => Arc::new(UnavailableWhipBackend(e.to_string())),
    }
}

#[cfg(not(feature = "whip"))]
fn real_backend(_preview: PreviewHub) -> Arc<dyn WhipBackend> {
    Arc::new(UnavailableWhipBackend(
        "WHIP support not compiled in (build with --features whip)".into(),
    ))
}

/// Runs `whipserversrc` on a private loopback port per slot and relays offers to it; the public endpoint stays axum.
#[cfg(feature = "whip")]
pub mod gst_whip {
    use gstreamer as gst;
    use gstreamer::prelude::*;

    use super::*;
    use crate::whip_server::{http_request, HttpResponse};

    const INTERNAL_BASE_PORT: u16 = 19000;
    const ENDPOINT_PATH: &str = "/whip/endpoint";
    const CONNECT_ATTEMPTS: u32 = 30;

    /// Per-slot pipeline text; `port` is the element's private loopback signalling port.
    pub fn whip_source_description(port: u16, codec: WhipCodec) -> String {
        let codec = match codec {
            WhipCodec::H264Baseline => "H264",
            WhipCodec::Vp8 => "VP8",
        };
        format!(
            "whipserversrc video-codecs=<{codec}> stun-server=\"\" signaller::host-addr=http://127.0.0.1:{port} ! queue ! videoconvert ! videoscale name=scale add-borders=true ! video/x-raw,format=RGBA,width=320,height=180 ! appsink name=sink max-buffers=2 drop=true sync=false"
        )
    }

    struct Live {
        pipeline: gst::Pipeline,
        addr: SocketAddr,
        resource: String,
    }

    pub struct GstWhipBackend {
        preview: PreviewHub,
        live: Mutex<HashMap<String, Live>>,
        listener: Arc<Mutex<Option<IceListener>>>,
    }

    impl GstWhipBackend {
        pub fn new(preview: PreviewHub) -> Result<Self, AppError> {
            gst::init().map_err(|e| AppError::Internal(format!("GStreamer init failed: {e}")))?;
            if gst::ElementFactory::find("whipserversrc").is_none() {
                return Err(AppError::Internal(
                    "GStreamer plugin gst-plugin-webrtc (whipserversrc) is not installed".into(),
                ));
            }
            Ok(Self {
                preview,
                live: Mutex::new(HashMap::new()),
                listener: Arc::new(Mutex::new(None)),
            })
        }

        /// Forwards the inner webrtcbin ICE state to the listener, off the streaming thread.
        fn watch_ice(&self, pipeline: &gst::Pipeline, slot: &str) {
            let (listener, slot) = (Arc::clone(&self.listener), slot.to_string());
            pipeline.connect_deep_element_added(move |_, _, element| {
                let is_webrtcbin = element.factory().is_some_and(|f| f.name() == "webrtcbin");
                if !is_webrtcbin {
                    return;
                }
                let (listener, slot) = (Arc::clone(&listener), slot.clone());
                element.connect_notify(Some("ice-connection-state"), move |el, _| {
                    let value = el.property_value("ice-connection-state");
                    let nick = gst::glib::EnumValue::from_value(&value)
                        .map(|(_, v)| v.nick().to_string())
                        .unwrap_or_default();
                    let connected = match nick.as_str() {
                        "connected" | "completed" => true,
                        "disconnected" | "failed" => false,
                        _ => return,
                    };
                    let Some(l) = lock(&listener).clone() else {
                        return;
                    };
                    let slot = slot.clone();
                    // The service takes its own locks; never call it from a GStreamer thread.
                    std::thread::spawn(move || l(&slot, connected));
                });
            });
        }
    }

    /// Retries while the element HTTP server is still starting.
    fn relay(
        addr: SocketAddr,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<HttpResponse, AppError> {
        tokio::runtime::Handle::current().block_on(async {
            let mut last = AppError::Internal("no attempt made".into());
            for _ in 0..CONNECT_ATTEMPTS {
                match http_request(addr, method, path, headers, body).await {
                    Ok(r) => return Ok(r),
                    Err(e) => last = e,
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(last)
        })
    }

    impl WhipBackend for GstWhipBackend {
        fn negotiate(
            &self,
            slot: &str,
            offer: &str,
            choice: &CodecChoice,
        ) -> Result<String, AppError> {
            let n: u16 = slot.trim_start_matches("camera").parse().unwrap_or(0);
            let port = INTERNAL_BASE_PORT + n;
            let pipeline = gst::parse::launch(&whip_source_description(port, choice.codec))
                .map_err(|e| AppError::Internal(format!("invalid WHIP pipeline: {e}")))?
                .downcast::<gst::Pipeline>()
                .map_err(|_| AppError::Internal("WHIP pipeline is not a pipeline".into()))?;
            crate::pipeline::gst_backend::attach_preview(&pipeline, slot, self.preview.clone());
            self.watch_ice(&pipeline, slot);
            pipeline
                .set_state(gst::State::Playing)
                .map_err(|e| AppError::Internal(format!("WHIP pipeline failed to start: {e}")))?;
            let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
            let sdp = [("Content-Type", "application/sdp")];
            match relay(addr, "POST", ENDPOINT_PATH, &sdp, offer) {
                Ok(r) if r.status == 201 => {
                    let resource = r.header("location").unwrap_or_default().to_string();
                    lock(&self.live).insert(
                        slot.to_string(),
                        Live {
                            pipeline,
                            addr,
                            resource,
                        },
                    );
                    Ok(r.body)
                }
                other => {
                    let _ = pipeline.set_state(gst::State::Null);
                    Err(AppError::Internal(match other {
                        Ok(r) => format!("whipserversrc answered {} to the offer", r.status),
                        Err(e) => format!("whipserversrc is unreachable: {e}"),
                    }))
                }
            }
        }

        fn teardown(&self, slot: &str) {
            if let Some(live) = lock(&self.live).remove(slot) {
                let _ = live.pipeline.set_state(gst::State::Null);
            }
            self.preview.forget(slot);
        }

        fn ice_restart(&self, slot: &str, fragment: &str) -> Result<(), AppError> {
            let (addr, resource) = {
                let live = lock(&self.live);
                let l = live
                    .get(slot)
                    .ok_or_else(|| AppError::NotFound(format!("no live pipeline for {slot}")))?;
                (l.addr, l.resource.clone())
            };
            let headers = [("Content-Type", "application/trickle-ice-sdpfrag")];
            let r = relay(addr, "PATCH", &resource, &headers, fragment)?;
            if (200..300).contains(&r.status) {
                Ok(())
            } else {
                Err(AppError::Internal(format!(
                    "whipserversrc answered {} to the ICE restart",
                    r.status
                )))
            }
        }

        fn set_ice_listener(&self, listener: IceListener) {
            *lock(&self.listener) = Some(listener);
        }
    }

    impl Drop for GstWhipBackend {
        fn drop(&mut self) {
            for (_, live) in lock(&self.live).drain() {
                let _ = live.pipeline.set_state(gst::State::Null);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME_LIKE: &str = "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n\
m=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=mid:0\r\na=rtpmap:111 opus/48000/2\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96 97 98 99 100\r\na=mid:1\r\n\
a=rtpmap:96 VP8/90000\r\n\
a=rtpmap:97 H264/90000\r\na=fmtp:97 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=640c1f\r\n\
a=rtpmap:98 H264/90000\r\na=fmtp:98 level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42e01f\r\n\
a=rtpmap:99 H264/90000\r\na=fmtp:99 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f\r\n\
a=rtpmap:100 VP9/90000\r\n";

    fn offer(video_lines: &str) -> String {
        format!(
            "v=0\r\ns=-\r\nt=0 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 97\r\na=mid:v\r\n{video_lines}"
        )
    }

    #[test]
    fn slots_are_camera_n_only() {
        for ok in ["camera1", "camera4", "camera12"] {
            assert_eq!(parse_slot(ok).as_deref(), Some(ok));
        }
        for bad in [
            "",
            "camera",
            "camera0",
            "camera01",
            "camera100",
            "cam1",
            "camera1/..",
            "Camera1",
            "camera-1",
        ] {
            assert_eq!(parse_slot(bad), None, "{bad}");
        }
    }

    #[test]
    fn h264_baseline_is_preferred_over_vp8_and_mode_1_over_mode_0() {
        let c = select_codec(CHROME_LIKE).expect("codec");
        assert_eq!(c.codec, WhipCodec::H264Baseline);
        assert_eq!(c.payload_type, 99);
    }

    #[test]
    fn high_profile_h264_is_not_baseline_so_vp8_wins() {
        let sdp = offer("a=rtpmap:96 VP8/90000\r\na=rtpmap:97 H264/90000\r\na=fmtp:97 profile-level-id=640c1f;packetization-mode=1\r\n");
        assert_eq!(
            select_codec(&sdp).expect("codec"),
            CodecChoice {
                codec: WhipCodec::Vp8,
                payload_type: 96
            }
        );
    }

    #[test]
    fn missing_profile_defaults_to_baseline_and_constraint_set0_counts() {
        let bare = offer("a=rtpmap:97 H264/90000\r\n");
        assert_eq!(
            select_codec(&bare).expect("c").codec,
            WhipCodec::H264Baseline
        );
        let cs0 = offer("a=rtpmap:97 H264/90000\r\na=fmtp:97 profile-level-id=4d8028\r\n");
        assert_eq!(
            select_codec(&cs0).expect("c").codec,
            WhipCodec::H264Baseline
        );
        assert!(is_baseline("42e01f") && is_baseline("42001f"));
        assert!(!is_baseline("4d0028") && !is_baseline("640c1f") && !is_baseline("zz"));
        assert!(!is_baseline("42"));
    }

    #[test]
    fn unsupported_or_malformed_offers_are_rejected() {
        let only_vp9 = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 100\r\na=rtpmap:100 VP9/90000\r\n";
        assert!(matches!(
            select_codec(only_vp9),
            Err(WhipError::Unsupported(_))
        ));
        let audio_only = "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=rtpmap:111 opus/48000/2\r\n";
        assert!(matches!(
            select_codec(audio_only),
            Err(WhipError::BadRequest(_))
        ));
        assert!(matches!(
            select_codec("garbage"),
            Err(WhipError::BadRequest(_))
        ));
        // A codec in the rtpmap but absent from the m= line is not offered.
        let unlisted = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:97 H264/90000\r\n";
        assert!(matches!(
            select_codec(unlisted),
            Err(WhipError::Unsupported(_))
        ));
    }

    #[test]
    fn canned_answer_echoes_mid_payload_type_and_codec() {
        let c = select_codec(CHROME_LIKE).expect("codec");
        let a = canned_answer(CHROME_LIKE, &c);
        assert!(a.contains("a=mid:1\r\n") && a.contains("m=video 9 UDP/TLS/RTP/SAVPF 99\r\n"));
        assert!(a.contains("a=rtpmap:99 H264/90000") && a.contains("a=recvonly"));
        let vp8 = canned_answer(
            "m=video 9 X 96\r\n",
            &CodecChoice {
                codec: WhipCodec::Vp8,
                payload_type: 96,
            },
        );
        assert!(vp8.contains("a=rtpmap:96 VP8/90000") && vp8.contains("a=mid:0"));
    }

    #[test]
    fn five_second_interruption_recovers_without_operator_action() {
        let t0 = Instant::now();
        let mut m = LinkMachine::new(GRACE_PERIOD);
        assert_eq!(
            m.on_ice(false, t0 + Duration::from_secs(1)),
            Some(WhipLinkState::Disconnected)
        );
        assert_eq!(m.tick(t0 + Duration::from_secs(5)), None);
        assert_eq!(
            m.on_ice(true, t0 + Duration::from_secs(6)),
            Some(WhipLinkState::Connected)
        );
        assert_eq!(m.tick(t0 + Duration::from_secs(60)), None);
        assert_eq!(m.state(), WhipLinkState::Connected);
    }

    #[test]
    fn interruption_past_the_grace_period_is_lost_and_terminal() {
        let t0 = Instant::now();
        let mut m = LinkMachine::new(GRACE_PERIOD);
        m.on_ice(false, t0);
        assert_eq!(m.tick(t0 + Duration::from_millis(9_999)), None);
        assert_eq!(
            m.tick(t0 + Duration::from_secs(10)),
            Some(WhipLinkState::Lost)
        );
        assert_eq!(m.on_ice(true, t0 + Duration::from_secs(11)), None);
        assert_eq!(m.tick(t0 + Duration::from_secs(20)), None);
        assert_eq!(m.state(), WhipLinkState::Lost);
    }

    #[test]
    fn repeated_signals_do_not_restart_the_grace_clock() {
        let t0 = Instant::now();
        let mut m = LinkMachine::new(GRACE_PERIOD);
        assert!(m.on_ice(false, t0).is_some());
        assert_eq!(m.on_ice(false, t0 + Duration::from_secs(8)), None);
        assert!(m.on_ice(true, t0).is_some());
        assert_eq!(m.on_ice(true, t0), None);
        m.on_ice(false, t0 + Duration::from_secs(8));
        assert_eq!(m.tick(t0 + Duration::from_secs(17)), None);
    }

    #[test]
    fn lan_ip_prefers_private_v4_and_skips_loopback_and_link_local() {
        let ip = |s: &str| s.parse::<IpAddr>().expect("ip");
        assert_eq!(
            pick_lan_ip(&[ip("127.0.0.1"), ip("169.254.1.5"), ip("::1")]),
            None
        );
        assert_eq!(
            pick_lan_ip(&[ip("203.0.113.9"), ip("192.168.1.20")]),
            Some(ip("192.168.1.20"))
        );
        assert_eq!(pick_lan_ip(&[ip("203.0.113.9")]), Some(ip("203.0.113.9")));
        assert_eq!(
            pick_lan_ip(&[ip("0.0.0.0"), ip("10.1.2.3")]),
            Some(ip("10.1.2.3"))
        );
        assert_eq!(pick_lan_ip(&[]), None);
        assert_eq!(
            pick_lan_ip(&FixedLocalIp(vec![ip("172.16.0.4")]).candidates()),
            Some(ip("172.16.0.4"))
        );
    }

    #[test]
    fn url_format_matches_the_phone_facing_contract() {
        let v4: IpAddr = "192.168.1.20".parse().expect("ip");
        assert_eq!(
            whip_url(v4, 8088, "camera1"),
            "http://192.168.1.20:8088/whip/camera1"
        );
        let v6: IpAddr = "fd00::1".parse().expect("ip");
        assert_eq!(
            whip_url(v6, 80, "camera2"),
            "http://[fd00::1]:80/whip/camera2"
        );
    }

    #[test]
    fn qr_is_square_deterministic_and_has_finder_patterns() {
        let q = qr_matrix("http://192.168.1.20:8088/whip/camera1").expect("qr");
        assert!(q.size >= 21 && q.modules.len() == (q.size * q.size) as usize);
        assert_eq!(
            q,
            qr_matrix("http://192.168.1.20:8088/whip/camera1").expect("qr")
        );
        let at = |x: u32, y: u32| q.modules[(y * q.size + x) as usize];
        // Top-left finder: dark ring, light ring, dark 3x3 core.
        assert!((0..7).all(|i| at(i, 0) && at(0, i) && at(6, i) && at(i, 6)));
        assert!(!at(1, 1) && !at(5, 5) && at(3, 3));
        assert_ne!(
            q,
            qr_matrix("http://192.168.1.20:8088/whip/camera2").expect("qr")
        );
    }

    #[test]
    fn token_validation_and_constant_time_compare() {
        assert!(validate_token("abc-123_XYZ").is_ok());
        for bad in ["", "has space", "tab\t", "ünï", &"x".repeat(257)] {
            assert!(
                matches!(validate_token(bad), Err(AppError::InvalidInput(_))),
                "{bad}"
            );
        }
        assert!(tokens_match("secret", "secret"));
        assert!(
            !tokens_match("secret", "secre")
                && !tokens_match("secret", "secreT")
                && !tokens_match("", "x")
        );
    }

    #[test]
    fn memory_token_store_round_trips_and_can_fail() {
        let s = MemoryTokenStore::default();
        assert_eq!(s.get().expect("get"), None);
        s.set(Some("t")).expect("set");
        assert_eq!(s.get().expect("get").as_deref(), Some("t"));
        s.set(None).expect("clear");
        assert_eq!(s.get().expect("get"), None);
        s.failing(Some("locked"));
        assert!(matches!(s.get(), Err(AppError::Keychain(_))));
    }

    struct Rig {
        service: Arc<WhipService>,
        backend: SimulatedWhipBackend,
        rx: tokio::sync::broadcast::Receiver<AppEvent>,
    }

    fn rig() -> Rig {
        let (bus, _keep) = EventBus::new();
        let bus = Arc::new(bus);
        let backend = SimulatedWhipBackend::default();
        let rx = bus.subscribe();
        Rig {
            service: WhipService::new(Arc::new(backend.clone()), bus),
            backend,
            rx,
        }
    }

    impl Rig {
        fn events(&mut self) -> Vec<AppEvent> {
            let mut out = Vec::new();
            while let Ok(e) = self.rx.try_recv() {
                out.push(e);
            }
            out
        }
    }

    #[test]
    fn create_publishes_source_added_and_negotiates_through_the_backend() {
        let mut r = rig();
        let c = r
            .service
            .create_session("camera1", CHROME_LIKE)
            .expect("create");
        assert_eq!(c.codec, WhipCodec::H264Baseline);
        assert!(c.answer.contains("a=recvonly"));
        let ev = r.events();
        assert!(
            matches!(&ev[0], AppEvent::SourceAdded { name, format: StreamFormat::Standard } if name == "camera1")
        );
        assert!(
            matches!(&ev[1], AppEvent::WhipSessionChanged { slot, state: WhipLinkState::Connected } if slot == "camera1")
        );
        assert_eq!(r.backend.calls(), ["negotiate:camera1:H264Baseline"]);
    }

    #[test]
    fn a_live_slot_is_a_conflict_and_a_fifth_phone_is_busy() {
        let r = rig();
        r.service
            .create_session("camera1", CHROME_LIKE)
            .expect("c1");
        assert!(matches!(
            r.service.create_session("camera1", CHROME_LIKE),
            Err(WhipError::Conflict(_))
        ));
        for n in 2..=4 {
            r.service
                .create_session(&format!("camera{n}"), CHROME_LIKE)
                .expect("slot");
        }
        assert!(matches!(
            r.service.create_session("camera5", CHROME_LIKE),
            Err(WhipError::Busy(_))
        ));
        assert_eq!(r.service.sessions().len(), 4);
    }

    #[test]
    fn rejected_offers_and_backend_failures_leave_no_session() {
        let r = rig();
        assert!(matches!(
            r.service.create_session("camera1", "garbage"),
            Err(WhipError::BadRequest(_))
        ));
        r.backend.fail_next_negotiate("engine down");
        assert!(
            matches!(r.service.create_session("camera1", CHROME_LIKE), Err(WhipError::Internal(m)) if m.contains("engine down"))
        );
        assert!(r.service.sessions().is_empty());
        r.service
            .create_session("camera1", CHROME_LIKE)
            .expect("retry works");
    }

    #[test]
    fn unavailable_backend_answers_unavailable_before_parsing() {
        let (bus, _k) = EventBus::new();
        let s = WhipService::new(
            Arc::new(UnavailableWhipBackend("no engine".into())),
            Arc::new(bus),
        );
        assert!(
            matches!(s.create_session("camera1", "garbage"), Err(WhipError::Unavailable(m)) if m == "no engine")
        );
        assert_eq!(s.unavailable_reason().as_deref(), Some("no engine"));
    }

    #[test]
    fn delete_requires_the_session_id_and_publishes_removal() {
        let mut r = rig();
        let c = r
            .service
            .create_session("camera1", CHROME_LIKE)
            .expect("create");
        r.events();
        assert!(matches!(
            r.service.delete_session("camera1", "nope"),
            Err(WhipError::NotFound(_))
        ));
        assert!(matches!(
            r.service.delete_session("camera2", &c.session_id),
            Err(WhipError::NotFound(_))
        ));
        r.service
            .delete_session("camera1", &c.session_id)
            .expect("delete");
        assert!(matches!(&r.events()[0], AppEvent::SourceRemoved { name } if name == "camera1"));
        assert_eq!(
            r.backend.calls().last().map(String::as_str),
            Some("teardown:camera1")
        );
        assert!(matches!(
            r.service.delete_session("camera1", &c.session_id),
            Err(WhipError::NotFound(_))
        ));
    }

    #[test]
    fn interruption_survives_within_grace_and_loses_beyond_it() {
        let (bus, _k) = EventBus::new();
        let bus = Arc::new(bus);
        let mut rx = bus.subscribe();
        let backend = SimulatedWhipBackend::default();
        let s = WhipService::with_grace(Arc::new(backend.clone()), bus, GRACE_PERIOD);
        let t0 = Instant::now();
        let a = s.create_session("camera1", CHROME_LIKE).expect("a");
        let b = s.create_session("camera2", CHROME_LIKE).expect("b");
        s.ice_signal("camera1", &a.session_id, false, t0)
            .expect("down");
        s.ice_signal("camera2", &b.session_id, false, t0)
            .expect("down");
        s.tick(t0 + Duration::from_secs(5));
        s.restart_ice(
            "camera1",
            &a.session_id,
            "a=ice-ufrag:x",
            t0 + Duration::from_secs(5),
        )
        .expect("restart");
        s.tick(t0 + Duration::from_secs(11));
        let states: HashMap<String, WhipLinkState> = s
            .sessions()
            .into_iter()
            .map(|i| (i.slot, i.state))
            .collect();
        assert_eq!(states["camera1"], WhipLinkState::Connected);
        assert_eq!(states["camera2"], WhipLinkState::Lost);
        let mut seen = Vec::new();
        while let Ok(e) = rx.try_recv() {
            seen.push(e);
        }
        assert!(seen
            .iter()
            .any(|e| matches!(e, AppEvent::SourceLost { name } if name == "camera2")));
        assert!(!seen
            .iter()
            .any(|e| matches!(e, AppEvent::SourceLost { name } if name == "camera1")));
        assert!(backend.calls().contains(&"teardown:camera2".to_string()));
        assert!(!backend.calls().contains(&"teardown:camera1".to_string()));
        assert!(matches!(
            s.restart_ice("camera2", &b.session_id, "", t0),
            Err(WhipError::NotFound(_))
        ));
    }

    #[test]
    fn a_lost_or_disconnected_slot_can_be_taken_over_by_a_fresh_post() {
        let mut r = rig();
        let t0 = Instant::now();
        let a = r.service.create_session("camera1", CHROME_LIKE).expect("a");
        r.service
            .ice_signal("camera1", &a.session_id, false, t0)
            .expect("down");
        r.events();
        let b = r
            .service
            .create_session("camera1", CHROME_LIKE)
            .expect("takeover");
        assert_ne!(a.session_id, b.session_id);
        let ev = r.events();
        assert!(
            matches!(&ev[0], AppEvent::SourceDiscovered { source } if source.name == "camera1")
        );
        assert!(matches!(
            r.service.delete_session("camera1", &a.session_id),
            Err(WhipError::NotFound(_))
        ));
        assert_eq!(r.service.sessions()[0].state, WhipLinkState::Connected);
    }

    #[test]
    fn shutdown_all_removes_every_session() {
        let mut r = rig();
        r.service
            .create_session("camera1", CHROME_LIKE)
            .expect("c1");
        r.service
            .create_session("camera2", CHROME_LIKE)
            .expect("c2");
        r.events();
        r.service.shutdown_all();
        let removed = r
            .events()
            .iter()
            .filter(|e| matches!(e, AppEvent::SourceRemoved { .. }))
            .count();
        assert_eq!(removed, 2);
        assert!(r.service.sessions().is_empty());
    }

    #[test]
    fn backend_ice_reports_drive_the_session_without_any_http_call() {
        let mut r = rig();
        r.service
            .create_session("camera1", CHROME_LIKE)
            .expect("create");
        r.events();
        r.backend.simulate_ice("camera1", false);
        assert_eq!(r.service.sessions()[0].state, WhipLinkState::Disconnected);
        assert!(matches!(
            &r.events()[0],
            AppEvent::WhipSessionChanged {
                state: WhipLinkState::Disconnected,
                ..
            }
        ));
        r.backend.simulate_ice("camera1", true);
        assert_eq!(r.service.sessions()[0].state, WhipLinkState::Connected);
        r.backend.simulate_ice("camera9", false);
        assert_eq!(r.service.sessions().len(), 1);
    }

    #[test]
    fn authorize_accepts_only_the_exact_bearer_token_once_one_is_set() {
        let r = rig();
        assert!(r.service.authorize(None).is_ok());
        r.service.set_token(Some("s3cret".into()));
        assert!(r.service.token_set());
        assert_eq!(r.service.authorize(None), Err(WhipError::Unauthorized));
        assert_eq!(
            r.service.authorize(Some("Bearer wrong")),
            Err(WhipError::Unauthorized)
        );
        assert_eq!(
            r.service.authorize(Some("Basic s3cret")),
            Err(WhipError::Unauthorized)
        );
        assert_eq!(
            r.service.authorize(Some("s3cret")),
            Err(WhipError::Unauthorized)
        );
        assert!(r.service.authorize(Some("Bearer s3cret")).is_ok());
        assert!(r.service.authorize(Some("bearer s3cret")).is_ok());
        r.service.set_token(None);
        assert!(r.service.authorize(None).is_ok());
    }
}
