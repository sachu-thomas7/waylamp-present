use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::map_response;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use axum::Router;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::AppError;
use crate::whip::{parse_slot, WhipError, WhipService};

const MAX_BODY_BYTES: usize = 64 * 1024;
const ACCEPT_POST: HeaderName = HeaderName::from_static("accept-post");
const SDP: &str = "application/sdp";
const TRICKLE: &str = "application/trickle-ice-sdpfrag";

type Ctx = Arc<WhipService>;

impl IntoResponse for WhipError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Unsupported(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Busy(_) | Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut response = (status, self.to_string()).into_response();
        if self == Self::Unauthorized {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

/// Browser-based WHIP clients need CORS, including on error responses.
async fn cors(mut response: Response) -> Response {
    let h = response.headers_mut();
    let set = |h: &mut HeaderMap, name: HeaderName, value: &'static str| {
        h.insert(name, HeaderValue::from_static(value));
    };
    set(h, header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    set(
        h,
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "POST, PATCH, DELETE, OPTIONS",
    );
    set(
        h,
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "authorization, content-type, if-match",
    );
    set(
        h,
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        "location, link, etag",
    );
    response
}

fn authorize(ctx: &Ctx, headers: &HeaderMap) -> Result<(), WhipError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    ctx.authorize(value)
}

fn slot_of(raw: &str) -> Result<String, WhipError> {
    parse_slot(raw).ok_or_else(|| WhipError::NotFound(format!("unknown WHIP slot: {raw}")))
}

fn has_content_type(headers: &HeaderMap, wanted: &str) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case(wanted))
        })
}

fn body_text(body: Bytes) -> Result<String, WhipError> {
    String::from_utf8(body.to_vec()).map_err(|_| WhipError::BadRequest("body is not UTF-8".into()))
}

/// Backends may block, so every service call runs off the async threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, WhipError> + Send + 'static,
) -> Result<T, WhipError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| WhipError::Internal(format!("handler task failed: {e}")))?
}

async fn post_offer(
    State(ctx): State<Ctx>,
    Path(slot): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, WhipError> {
    authorize(&ctx, &headers)?;
    let slot = slot_of(&slot)?;
    if !has_content_type(&headers, SDP) {
        let mut r = (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/sdp",
        )
            .into_response();
        r.headers_mut()
            .insert(ACCEPT_POST, HeaderValue::from_static(SDP));
        return Ok(r);
    }
    let offer = body_text(body)?;
    let service = Arc::clone(&ctx);
    let slot_for_call = slot.clone();
    let created = blocking(move || service.create_session(&slot_for_call, &offer)).await?;
    let location = format!("/whip/{slot}/{}", created.session_id);
    Ok((
        StatusCode::CREATED,
        [
            (header::CONTENT_TYPE, SDP.to_string()),
            (header::LOCATION, location),
        ],
        created.answer,
    )
        .into_response())
}

async fn options_endpoint() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(header::ALLOW, "OPTIONS, POST"), (ACCEPT_POST, SDP)],
    )
}

async fn options_resource() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(header::ALLOW, "OPTIONS, PATCH, DELETE")],
    )
}

async fn delete_session(
    State(ctx): State<Ctx>,
    Path((slot, session)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, WhipError> {
    authorize(&ctx, &headers)?;
    let slot = slot_of(&slot)?;
    blocking(move || ctx.delete_session(&slot, &session)).await?;
    Ok(StatusCode::OK)
}

async fn patch_session(
    State(ctx): State<Ctx>,
    Path((slot, session)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, WhipError> {
    authorize(&ctx, &headers)?;
    let slot = slot_of(&slot)?;
    if !has_content_type(&headers, TRICKLE) {
        return Err(WhipError::BadRequest(format!(
            "Content-Type must be {TRICKLE}"
        )));
    }
    let fragment = body_text(body)?;
    blocking(move || ctx.restart_ice(&slot, &session, &fragment, Instant::now())).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router(service: Arc<WhipService>) -> Router {
    Router::new()
        .route("/whip/{slot}", post(post_offer).options(options_endpoint))
        .route(
            "/whip/{slot}/{session}",
            delete(delete_session)
                .patch(patch_session)
                .options(options_resource),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(map_response(cors))
        .with_state(service)
}

/// Serves `listener` until `shutdown` fires. Called from async code, so the Tauri runtime spawn is fine.
pub fn spawn(
    listener: tokio::net::TcpListener,
    service: Arc<WhipService>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let served = axum::serve(listener, router(service))
            .with_graceful_shutdown(async move {
                let _ = shutdown.await;
            })
            .await;
        if let Err(e) = served {
            eprintln!("WHIP server stopped: {e}");
        }
    })
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Minimal HTTP/1.1 client (one request, `Connection: close`): the tests' wire-level client and the Gst relay.
pub async fn http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Result<HttpResponse, AppError> {
    let io = |e: std::io::Error| AppError::Internal(format!("http request failed: {e}"));
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (k, v) in headers {
        request.push_str(&format!("{k}: {v}\r\n"));
    }
    request.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    let mut stream = tokio::net::TcpStream::connect(addr).await.map_err(io)?;
    stream.write_all(request.as_bytes()).await.map_err(io)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.map_err(io)?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| AppError::Internal("malformed HTTP response".into()))?;
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Internal("malformed HTTP status line".into()))?;
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    Ok(HttpResponse {
        status,
        headers,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::bus::EventBus;
    use crate::whip::SimulatedWhipBackend;

    const OFFER: &str = "v=0\r\ns=-\r\nt=0 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 97\r\na=mid:0\r\na=rtpmap:96 VP8/90000\r\na=rtpmap:97 H264/90000\r\na=fmtp:97 packetization-mode=1;profile-level-id=42e01f\r\n";
    const SDP_HEADER: (&str, &str) = ("Content-Type", "application/sdp");

    struct Server {
        addr: SocketAddr,
        service: Arc<WhipService>,
        backend: SimulatedWhipBackend,
        _shutdown: tokio::sync::oneshot::Sender<()>,
    }

    async fn serve() -> Server {
        let (bus, _keep) = EventBus::new();
        let backend = SimulatedWhipBackend::default();
        let service = WhipService::new(Arc::new(backend.clone()), Arc::new(bus));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (tx, rx) = tokio::sync::oneshot::channel();
        spawn(listener, Arc::clone(&service), rx);
        Server {
            addr,
            service,
            backend,
            _shutdown: tx,
        }
    }

    async fn req(
        s: &Server,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> HttpResponse {
        http_request(s.addr, method, path, headers, body)
            .await
            .expect("http")
    }

    #[tokio::test]
    async fn post_returns_201_location_and_an_sdp_answer() {
        let s = serve().await;
        let r = req(&s, "POST", "/whip/camera1", &[SDP_HEADER], OFFER).await;
        assert_eq!(r.status, 201);
        assert_eq!(r.header("content-type"), Some("application/sdp"));
        let location = r.header("location").expect("location");
        assert!(location.starts_with("/whip/camera1/") && location.len() > "/whip/camera1/".len());
        assert!(r.body.contains("a=recvonly") && r.body.contains("H264/90000"));
        assert_eq!(s.service.sessions().len(), 1);
    }

    #[tokio::test]
    async fn options_advertises_post_and_cors_without_auth() {
        let s = serve().await;
        s.service.set_token(Some("tok".into()));
        let r = req(&s, "OPTIONS", "/whip/camera1", &[], "").await;
        assert_eq!(r.status, 204);
        assert_eq!(r.header("allow"), Some("OPTIONS, POST"));
        assert_eq!(r.header("accept-post"), Some("application/sdp"));
        assert_eq!(r.header("access-control-allow-origin"), Some("*"));
        assert!(r
            .header("access-control-allow-headers")
            .is_some_and(|h| h.contains("authorization")));
        let res = req(&s, "OPTIONS", "/whip/camera1/abc", &[], "").await;
        assert_eq!(
            (res.status, res.header("allow")),
            (204, Some("OPTIONS, PATCH, DELETE"))
        );
    }

    #[tokio::test]
    async fn delete_tears_the_session_down_and_frees_the_slot() {
        let s = serve().await;
        let r = req(&s, "POST", "/whip/camera2", &[SDP_HEADER], OFFER).await;
        let location = r.header("location").expect("location").to_string();
        assert_eq!(
            req(&s, "DELETE", "/whip/camera2/wrong", &[], "")
                .await
                .status,
            404
        );
        assert_eq!(req(&s, "DELETE", &location, &[], "").await.status, 200);
        assert!(s.service.sessions().is_empty());
        assert!(s.backend.calls().contains(&"teardown:camera2".to_string()));
        assert_eq!(req(&s, "DELETE", &location, &[], "").await.status, 404);
        assert_eq!(
            req(&s, "POST", "/whip/camera2", &[SDP_HEADER], OFFER)
                .await
                .status,
            201
        );
    }

    #[tokio::test]
    async fn request_validation_maps_to_the_right_status_codes() {
        let s = serve().await;
        let wrong_type = req(
            &s,
            "POST",
            "/whip/camera1",
            &[("Content-Type", "text/plain")],
            OFFER,
        )
        .await;
        assert_eq!(wrong_type.status, 415);
        assert_eq!(wrong_type.header("accept-post"), Some("application/sdp"));
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &[], OFFER).await.status,
            415
        );
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &[SDP_HEADER], "garbage")
                .await
                .status,
            400
        );
        let vp9 = "v=0\r\nm=video 9 X 100\r\na=rtpmap:100 VP9/90000\r\n";
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &[SDP_HEADER], vp9)
                .await
                .status,
            422
        );
        assert_eq!(
            req(&s, "POST", "/whip/cam1", &[SDP_HEADER], OFFER)
                .await
                .status,
            404
        );
        assert_eq!(
            req(&s, "POST", "/whip/camera0", &[SDP_HEADER], OFFER)
                .await
                .status,
            404
        );
        assert_eq!(req(&s, "GET", "/whip/camera1", &[], "").await.status, 405);
        assert_eq!(req(&s, "GET", "/nope", &[], "").await.status, 404);
        assert!(s.service.sessions().is_empty());
        let big = "a".repeat(MAX_BODY_BYTES + 1);
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &[SDP_HEADER], &big)
                .await
                .status,
            413
        );
    }

    #[tokio::test]
    async fn live_slot_conflicts_and_the_fifth_phone_is_refused() {
        let s = serve().await;
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &[SDP_HEADER], OFFER)
                .await
                .status,
            201
        );
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &[SDP_HEADER], OFFER)
                .await
                .status,
            409
        );
        for n in 2..=4 {
            let r = req(
                &s,
                "POST",
                &format!("/whip/camera{n}"),
                &[SDP_HEADER],
                OFFER,
            )
            .await;
            assert_eq!(r.status, 201, "camera{n}");
        }
        assert_eq!(
            req(&s, "POST", "/whip/camera5", &[SDP_HEADER], OFFER)
                .await
                .status,
            503
        );
        assert_eq!(s.service.sessions().len(), 4);
    }

    #[tokio::test]
    async fn bearer_token_gates_post_delete_and_patch_but_not_preflight() {
        let s = serve().await;
        s.service.set_token(Some("s3cret".into()));
        let denied = req(&s, "POST", "/whip/camera1", &[SDP_HEADER], OFFER).await;
        assert_eq!(denied.status, 401);
        assert_eq!(denied.header("www-authenticate"), Some("Bearer"));
        let wrong = [SDP_HEADER, ("Authorization", "Bearer nope")];
        assert_eq!(
            req(&s, "POST", "/whip/camera1", &wrong, OFFER).await.status,
            401
        );
        assert!(s.service.sessions().is_empty());
        let ok = [SDP_HEADER, ("Authorization", "Bearer s3cret")];
        let r = req(&s, "POST", "/whip/camera1", &ok, OFFER).await;
        assert_eq!(r.status, 201);
        let loc = r.header("location").expect("location").to_string();
        assert_eq!(req(&s, "DELETE", &loc, &[], "").await.status, 401);
        assert_eq!(s.service.sessions().len(), 1);
        assert_eq!(
            req(
                &s,
                "DELETE",
                &loc,
                &[("Authorization", "Bearer s3cret")],
                ""
            )
            .await
            .status,
            200
        );
    }

    #[tokio::test]
    async fn patch_is_an_ice_restart_that_brings_a_disconnected_session_back() {
        let s = serve().await;
        let r = req(&s, "POST", "/whip/camera1", &[SDP_HEADER], OFFER).await;
        let loc = r.header("location").expect("location").to_string();
        let id = loc.rsplit('/').next().expect("id").to_string();
        s.service
            .ice_signal("camera1", &id, false, Instant::now())
            .expect("down");
        assert_eq!(
            s.service.sessions()[0].state,
            crate::whip::WhipLinkState::Disconnected
        );
        let frag = ("Content-Type", "application/trickle-ice-sdpfrag");
        assert_eq!(
            req(&s, "PATCH", &loc, &[], "a=ice-ufrag:x").await.status,
            400
        );
        assert_eq!(
            req(&s, "PATCH", &loc, &[frag], "a=ice-ufrag:x")
                .await
                .status,
            204
        );
        assert_eq!(
            s.service.sessions()[0].state,
            crate::whip::WhipLinkState::Connected
        );
        assert!(s
            .backend
            .calls()
            .contains(&"ice_restart:camera1".to_string()));
        assert_eq!(
            req(&s, "PATCH", "/whip/camera1/other", &[frag], "x")
                .await
                .status,
            404
        );
    }

    #[tokio::test]
    async fn graceful_shutdown_stops_accepting_connections() {
        let (bus, _keep) = EventBus::new();
        let service = WhipService::new(Arc::new(SimulatedWhipBackend::default()), Arc::new(bus));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = spawn(listener, service, rx);
        assert_eq!(
            http_request(addr, "OPTIONS", "/whip/camera1", &[], "")
                .await
                .expect("up")
                .status,
            204
        );
        tx.send(()).expect("signal");
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("server stops")
            .expect("task");
        assert!(http_request(addr, "OPTIONS", "/whip/camera1", &[], "")
            .await
            .is_err());
    }
}
