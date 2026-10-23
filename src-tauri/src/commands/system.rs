use crate::error::AppError;

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct PingResponse {
    pub ok: bool,
    pub version: String,
}

#[tauri::command]
pub async fn ping() -> Result<PingResponse, AppError> {
    Ok(PingResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn ping_returns_ok_and_version() {
        let result = ping().await.expect("ping should not fail");
        assert!(result.ok);
        assert!(!result.version.is_empty());
    }

    #[tokio::test]
    async fn ping_dispatches_under_2ms() {
        // Best of 20: a loaded box must not fail a latency check on scheduler noise.
        let mut elapsed = std::time::Duration::MAX;
        for _ in 0..20 {
            let start = Instant::now();
            let _ = ping().await.expect("ping should not fail");
            elapsed = elapsed.min(start.elapsed());
        }
        assert!(elapsed.as_millis() < 2, "took {}µs", elapsed.as_micros());
    }
}
