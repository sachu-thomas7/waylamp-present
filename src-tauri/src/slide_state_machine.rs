use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::slide::{SlideDeck, SlidePayload};

// Typed persistence struct — backwards compatible (timer_started_at defaults to None)
#[derive(serde::Serialize, serde::Deserialize)]
struct SlideState {
    current_index: usize,
    #[serde(default)]
    timer_started_at: Option<u64>, // Unix ms; None = timer not yet started
    #[serde(default)]
    saved_at: Option<u64>, // Unix ms of this write; a stale file must not resurrect an old timer
}

/// A timer older than this at restore time belongs to a previous session, not a crash mid-service.
const TIMER_RESTORE_MAX_AGE_MS: u64 = 6 * 60 * 60 * 1000;

fn restorable_timer_start(
    started_at: Option<u64>,
    saved_at: Option<u64>,
    now_ms: u64,
) -> Option<u64> {
    let (started, saved) = (started_at?, saved_at?);
    (saved <= now_ms && now_ms - saved <= TIMER_RESTORE_MAX_AGE_MS).then_some(started)
}

pub struct SlideStateMachine {
    deck: SlideDeck,
    current_index: usize,
    data_dir: PathBuf,
    timer_started_at: Option<u64>,
}

impl SlideStateMachine {
    pub fn load_deck(&mut self, deck: crate::slide::SlideDeck) {
        self.deck = deck;
        self.current_index = 0;
        self.timer_started_at = None;
    }

    pub fn new(deck: SlideDeck, data_dir: PathBuf) -> Self {
        Self {
            deck,
            current_index: 0,
            data_dir,
            timer_started_at: None,
        }
    }

    pub fn current_payload(&self) -> SlidePayload {
        match self.deck.get(self.current_index) {
            Some(entry) => SlidePayload::from_entry(entry, self.deck.len() as u32),
            None => SlidePayload::empty(),
        }
    }

    pub fn current_index(&self) -> usize {
        self.current_index
    }
    pub fn total(&self) -> usize {
        self.deck.len()
    }
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
    pub fn timer_started_at(&self) -> Option<u64> {
        self.timer_started_at
    }

    pub fn deck_len(&self) -> usize {
        self.deck.slides.len()
    }

    pub fn all_slides(&self) -> &[crate::slide::SlideEntry] {
        &self.deck.slides
    }
}

pub async fn advance(handle: &StateMachineHandle, bus: &Arc<EventBus>) -> Result<(), AppError> {
    let (payload, data_dir) = {
        let mut sm = handle.write().await;
        if sm.current_index + 1 >= sm.total() {
            bus.publish(AppEvent::DeckEnd);
            return Ok(());
        }
        sm.current_index += 1;
        let p = sm.current_payload();
        let d = sm.data_dir().to_path_buf();
        (p, d)
    };
    persist_index(handle, &data_dir).await?;
    start_timer_if_needed(handle, &data_dir).await?;
    bus.publish(AppEvent::SlideAdvance);
    bus.publish(AppEvent::SlideUpdate(payload));
    Ok(())
}

pub async fn rewind(handle: &StateMachineHandle, bus: &Arc<EventBus>) -> Result<(), AppError> {
    let (payload, data_dir) = {
        let mut sm = handle.write().await;
        if sm.current_index == 0 {
            bus.publish(AppEvent::DeckStart);
            return Ok(());
        }
        sm.current_index -= 1;
        let p = sm.current_payload();
        let d = sm.data_dir().to_path_buf();
        (p, d)
    };
    persist_index(handle, &data_dir).await?;
    // Timer intentionally NOT reset or restarted on rewind
    bus.publish(AppEvent::SlideRewind);
    bus.publish(AppEvent::SlideUpdate(payload));
    Ok(())
}

pub type StateMachineHandle = Arc<RwLock<SlideStateMachine>>;

pub fn new_handle(deck: SlideDeck, data_dir: PathBuf) -> StateMachineHandle {
    Arc::new(RwLock::new(SlideStateMachine::new(deck, data_dir)))
}

pub async fn jump_to(
    index: usize,
    handle: &StateMachineHandle,
    bus: &Arc<EventBus>,
) -> Result<(), AppError> {
    let (payload, data_dir) = {
        let mut sm = handle.write().await;
        if index >= sm.total() {
            return Err(AppError::InvalidInput(format!(
                "slide index {index} out of range (deck has {} slides)",
                sm.total()
            )));
        }
        sm.current_index = index;
        let p = sm.current_payload();
        let d = sm.data_dir().to_path_buf();
        (p, d)
    };
    persist_index(handle, &data_dir).await?;
    bus.publish(AppEvent::SlideUpdate(payload));
    Ok(())
}

pub async fn reset_timer(handle: &StateMachineHandle, bus: &Arc<EventBus>) -> Result<(), AppError> {
    let data_dir = {
        let sm = handle.read().await;
        sm.data_dir().to_path_buf()
    };
    let now_ms = now_unix_ms()?;
    let previous = {
        let mut sm = handle.write().await;
        sm.timer_started_at.replace(now_ms)
    };
    if let Err(e) = persist_index(handle, &data_dir).await {
        handle.write().await.timer_started_at = previous;
        return Err(e);
    }
    bus.publish(AppEvent::TimerReset);
    bus.publish(AppEvent::TimerTick { elapsed_secs: 0 });
    Ok(())
}

pub fn spawn_timer_task(
    handle: StateMachineHandle,
    bus: Arc<EventBus>,
) -> tokio::task::AbortHandle {
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            let started_at = handle.read().await.timer_started_at();
            if let Some(started_at) = started_at {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let elapsed_secs = now_ms.saturating_sub(started_at) / 1000;
                bus.publish(AppEvent::TimerTick { elapsed_secs });
            }
            // If timer_started_at is None: emit nothing — frontend shows "--:--"
        }
    });
    task.abort_handle()
}

pub async fn persist_state(handle: &StateMachineHandle, data_dir: &Path) -> Result<(), AppError> {
    persist_index(handle, data_dir).await
}

pub async fn restore_state(handle: &StateMachineHandle, data_dir: &Path) {
    let path = data_dir.join("slide_state.json");
    let (Some(state), _) =
        crate::persist::load_recovering(&path, |s| serde_json::from_str::<SlideState>(s).ok())
    else {
        return;
    };
    let mut sm = handle.write().await;
    if state.current_index < sm.total() {
        sm.current_index = state.current_index;
    }
    sm.timer_started_at = restorable_timer_start(
        state.timer_started_at,
        state.saved_at,
        now_unix_ms().unwrap_or(0),
    );
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn now_unix_ms() -> Result<u64, AppError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|e| AppError::Internal(format!("system clock error: {e}")))
}

async fn start_timer_if_needed(
    handle: &StateMachineHandle,
    data_dir: &Path,
) -> Result<(), AppError> {
    let already_started = handle.read().await.timer_started_at().is_some();
    if already_started {
        return Ok(());
    }
    let now_ms = now_unix_ms()?;
    {
        let mut sm = handle.write().await;
        sm.timer_started_at = Some(now_ms);
    }
    persist_index(handle, data_dir).await?;
    Ok(())
}

async fn persist_index(handle: &StateMachineHandle, data_dir: &Path) -> Result<(), AppError> {
    let (index, timer_started_at) = {
        let sm = handle.read().await;
        (sm.current_index(), sm.timer_started_at())
    };
    let state = SlideState {
        current_index: index,
        timer_started_at,
        saved_at: Some(now_unix_ms()?),
    };
    let json = serde_json::to_string(&state)
        .map_err(|e| AppError::Internal(format!("serialize slide state: {e}")))?;
    crate::persist::atomic_write(&data_dir.join("slide_state.json"), json.as_bytes())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::EventBus;
    use crate::slide::SlideDeck;
    use tempfile::tempdir;

    fn make_handle() -> (StateMachineHandle, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        (handle, dir)
    }

    fn make_bus() -> (Arc<EventBus>, tokio::sync::broadcast::Receiver<AppEvent>) {
        let (bus, rx) = EventBus::new();
        (Arc::new(bus), rx)
    }

    #[tokio::test]
    async fn advance_from_last_emits_deck_end() {
        let (handle, _dir) = make_handle();
        let (bus, mut rx) = make_bus();
        for _ in 0..4 {
            advance(&handle, &bus).await.unwrap();
            while rx.try_recv().is_ok() {}
        }
        advance(&handle, &bus).await.unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::DeckEnd));
        assert_eq!(handle.read().await.current_index(), 4);
    }

    #[tokio::test]
    async fn rewind_from_first_emits_deck_start() {
        let (handle, _dir) = make_handle();
        let (bus, mut rx) = make_bus();
        rewind(&handle, &bus).await.unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::DeckStart));
        assert_eq!(handle.read().await.current_index(), 0);
    }

    #[tokio::test]
    async fn jump_to_valid_index_publishes_slide_update() {
        let (handle, _dir) = make_handle();
        let (bus, mut rx) = make_bus();
        jump_to(3, &handle, &bus).await.unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::SlideUpdate(ref p) if p.index == 3));
        assert_eq!(handle.read().await.current_index(), 3);
    }

    #[tokio::test]
    async fn jump_to_invalid_index_returns_error() {
        let (handle, _dir) = make_handle();
        let (bus, _rx) = make_bus();
        let before = handle.read().await.current_index();
        let result = jump_to(99, &handle, &bus).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
        assert_eq!(handle.read().await.current_index(), before);
    }

    #[tokio::test]
    async fn advance_then_persist_then_restore_preserves_index() {
        let dir = tempdir().unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        let (bus, mut rx) = make_bus();
        advance(&handle, &bus).await.unwrap();
        advance(&handle, &bus).await.unwrap();
        advance(&handle, &bus).await.unwrap();
        while rx.try_recv().is_ok() {}
        persist_state(&handle, dir.path()).await.unwrap();
        let handle2 = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        assert_eq!(handle2.read().await.current_index(), 0);
        restore_state(&handle2, dir.path()).await;
        assert_eq!(handle2.read().await.current_index(), 3);
    }

    #[tokio::test]
    async fn timer_starts_on_first_advance_only() {
        let (handle, _dir) = make_handle();
        let (bus, _rx) = make_bus();
        assert!(handle.read().await.timer_started_at().is_none());
        advance(&handle, &bus).await.unwrap();
        let t1 = handle.read().await.timer_started_at();
        assert!(t1.is_some());
        advance(&handle, &bus).await.unwrap();
        assert_eq!(handle.read().await.timer_started_at(), t1);
    }

    #[tokio::test]
    async fn timer_does_not_reset_on_rewind() {
        let (handle, _dir) = make_handle();
        let (bus, _rx) = make_bus();
        advance(&handle, &bus).await.unwrap();
        let t1 = handle.read().await.timer_started_at();
        rewind(&handle, &bus).await.unwrap();
        assert_eq!(handle.read().await.timer_started_at(), t1);
    }

    #[tokio::test]
    async fn timer_reset_command_sets_new_start_time() {
        let (handle, _dir) = make_handle();
        let (bus, _rx) = make_bus();
        advance(&handle, &bus).await.unwrap();
        let t1 = handle.read().await.timer_started_at();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        reset_timer(&handle, &bus).await.unwrap();
        let t2 = handle.read().await.timer_started_at();
        assert!(t2 > t1);
    }

    #[tokio::test]
    async fn timer_persists_and_restores_correctly() {
        let dir = tempdir().unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        let (bus, _rx) = make_bus();
        advance(&handle, &bus).await.unwrap();
        let t1 = handle.read().await.timer_started_at();
        let handle2 = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        restore_state(&handle2, dir.path()).await;
        assert_eq!(handle2.read().await.timer_started_at(), t1);
    }

    #[tokio::test]
    async fn timer_tick_emits_correct_elapsed_after_restore() {
        let dir = tempdir().unwrap();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let started_at = now_ms - 30_000; // 30 seconds ago
        let state = SlideState {
            current_index: 0,
            timer_started_at: Some(started_at),
            saved_at: Some(now_ms),
        };
        std::fs::write(
            dir.path().join("slide_state.json"),
            serde_json::to_string(&state).unwrap(),
        )
        .unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        restore_state(&handle, dir.path()).await;
        let restored = handle.read().await.timer_started_at().unwrap();
        let elapsed = (now_ms - restored) / 1000;
        assert!(elapsed >= 30, "elapsed {elapsed} should be >= 30");
    }

    const HOUR: u64 = 60 * 60 * 1000;

    #[test]
    fn restorable_timer_rules() {
        let now = 100 * HOUR;
        assert_eq!(
            restorable_timer_start(Some(now - HOUR), Some(now - 1000), now),
            Some(now - HOUR)
        );
        assert_eq!(restorable_timer_start(None, Some(now), now), None);
        assert_eq!(
            restorable_timer_start(Some(1), None, now),
            None,
            "pre-saved_at files are stale"
        );
        assert_eq!(
            restorable_timer_start(Some(1), Some(now - 7 * HOUR), now),
            None
        );
        assert_eq!(
            restorable_timer_start(Some(1), Some(now - 6 * HOUR), now),
            Some(1)
        );
        assert_eq!(
            restorable_timer_start(Some(1), Some(now + HOUR), now),
            None,
            "clock went backwards"
        );
    }

    #[tokio::test]
    async fn stale_timer_from_a_previous_session_is_not_resurrected() {
        let dir = tempdir().unwrap();
        let nine_days_ago = now_unix_ms().unwrap() - 9 * 24 * HOUR;
        std::fs::write(
            dir.path().join("slide_state.json"),
            format!(
                r#"{{"current_index":2,"timer_started_at":{nine_days_ago},"saved_at":{nine_days_ago}}}"#
            ),
        )
        .unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        restore_state(&handle, dir.path()).await;
        assert_eq!(handle.read().await.timer_started_at(), None);
        assert_eq!(
            handle.read().await.current_index(),
            2,
            "position still restored"
        );
    }

    #[tokio::test]
    async fn legacy_state_without_saved_at_drops_its_timer() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("slide_state.json"),
            r#"{"current_index":1,"timer_started_at":1700000000000}"#,
        )
        .unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        restore_state(&handle, dir.path()).await;
        assert_eq!(handle.read().await.timer_started_at(), None);
    }

    #[tokio::test]
    async fn corrupt_slide_state_falls_back_to_the_previous_save() {
        let dir = tempdir().unwrap();
        let handle = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        let (bus, _rx) = make_bus();
        advance(&handle, &bus).await.unwrap();
        advance(&handle, &bus).await.unwrap();
        std::fs::write(
            dir.path().join("slide_state.json"),
            "{\"current_index\": 9, trunc",
        )
        .unwrap();
        let fresh = new_handle(SlideDeck::stub(), dir.path().to_path_buf());
        restore_state(&fresh, dir.path()).await;
        assert_eq!(
            fresh.read().await.current_index(),
            1,
            "restored from .bak, one save behind"
        );
    }

    #[test]
    fn current_payload_on_empty_deck_returns_sentinel_not_panic() {
        use crate::slide::SlideDeck;
        let machine = SlideStateMachine {
            deck: SlideDeck::empty(),
            current_index: 0,
            data_dir: std::path::PathBuf::from("/tmp"),
            timer_started_at: None,
        };
        let payload = machine.current_payload();
        assert_eq!(payload.total, 0);
        assert_eq!(payload.title, "");
    }

    #[test]
    fn load_deck_resets_index_and_timer() {
        use crate::slide::SlideDeck;
        // Use a data_dir that won't actually write during test
        let mut machine = SlideStateMachine {
            deck: SlideDeck::stub(),
            current_index: 3,
            data_dir: std::path::PathBuf::from("/tmp"),
            timer_started_at: Some(12345),
        };
        machine.load_deck(SlideDeck::stub());
        assert_eq!(machine.current_index, 0);
        assert!(machine.timer_started_at.is_none());
    }
}
